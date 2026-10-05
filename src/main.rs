use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

#[derive(Parser)]
#[command(name = "meta", version, about = "Manage multiple Git repositories in parallel")]
struct Cli {
    #[arg(short, long, default_value = "meta.yaml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone missing repositories and pull existing repositories.
    Sync {
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Run a command in every selected repository.
    Exec {
        #[arg(long = "tag")]
        tags: Vec<String>,

        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Show repositories selected from the config.
    List {
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
struct Config {
    version: u32,
    repositories: Vec<Repository>,
}

#[derive(Debug, Deserialize)]
struct Repository {
    name: String,
    url: String,
    #[serde(default)]
    tags: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config(&cli.config)?;
    let root = cli.config.parent().unwrap_or(Path::new("."));

    if config.version != 1 {
        bail!("unsupported config version: {}", config.version);
    }

    match cli.command {
        Commands::Sync { tags } => {
            run_parallel(selected(&config.repositories, &tags), |repo| sync(root, repo))
        }
        Commands::Exec { tags, command } => {
            run_parallel(selected(&config.repositories, &tags), |repo| exec(root, repo, &command))
        }
        Commands::List { tags } => {
            for repo in selected(&config.repositories, &tags) {
                println!("{}\t{}\t{}", repo.name, repo.url, repo.tags.join(","));
            }
            Ok(())
        }
    }
}

fn load_config(path: &Path) -> Result<Config> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn selected<'a>(repos: &'a [Repository], tags: &[String]) -> Vec<&'a Repository> {
    if tags.is_empty() {
        return repos.iter().collect();
    }

    let wanted: HashSet<&str> = tags.iter().map(String::as_str).collect();
    repos
        .iter()
        .filter(|repo| repo.tags.iter().any(|tag| wanted.contains(tag.as_str())))
        .collect()
}

fn run_parallel<F>(repos: Vec<&Repository>, action: F) -> Result<()>
where
    F: Fn(&Repository) -> Result<()> + Sync,
{
    let failures: Vec<String> = repos
        .par_iter()
        .filter_map(|repo| match action(repo) {
            Ok(()) => None,
            Err(error) => {
                eprintln!("[{}] ERROR: {error:#}", repo.name);
                Some(repo.name.clone())
            }
        })
        .collect();

    if failures.is_empty() {
        Ok(())
    } else {
        bail!("failed repositories: {}", failures.join(", "))
    }
}

fn sync(root: &Path, repo: &Repository) -> Result<()> {
    let dir = root.join(&repo.name);
    if dir.join(".git").exists() {
        println!("[{}] pulling", repo.name);
        command("git", &["-C", path(&dir)?, "pull", "--ff-only"], None)
    } else if dir.exists() {
        bail!("{} exists but is not a Git repository", dir.display())
    } else {
        println!("[{}] cloning {}", repo.name, repo.url);
        command("git", &["clone", &repo.url, path(&dir)?], None)
    }
}

fn exec(root: &Path, repo: &Repository, args: &[String]) -> Result<()> {
    let dir = root.join(&repo.name);
    if !dir.join(".git").exists() {
        bail!("{} is not cloned; run 'meta sync' first", repo.name);
    }

    println!("[{}] $ {}", repo.name, args.join(" "));
    let (program, program_args) = args.split_first().context("command is empty")?;
    let refs: Vec<&str> = program_args.iter().map(String::as_str).collect();
    command(program, &refs, Some(&dir))
}

fn command(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    let status = cmd
        .status()
        .with_context(|| format!("failed to start {program}"))?;

    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

fn path(path: &Path) -> Result<&str> {
    path.to_str().context("repository path is not valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repos() -> Vec<Repository> {
        vec![
            Repository {
                name: "api".into(),
                url: "https://github.com/example/api.git".into(),
                tags: vec!["backend".into(), "rust".into()],
            },
            Repository {
                name: "web".into(),
                url: "https://github.com/example/web.git".into(),
                tags: vec!["frontend".into()],
            },
        ]
    }

    #[test]
    fn no_tags_selects_all() {
        assert_eq!(selected(&repos(), &[]).len(), 2);
    }

    #[test]
    fn tag_selects_matching_repositories() {
        let repos = repos();
        let tags = vec!["backend".to_string()];
        let result = selected(&repos, &tags);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "api");
    }
}
