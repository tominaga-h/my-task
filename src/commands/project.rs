use clap::Args;

use crate::config;
use crate::db;

#[derive(Args)]
pub struct ProjectArgs {
    /// Project name to configure
    pub name: String,

    /// Set the project's category
    #[arg(long)]
    pub set_category: Option<String>,

    /// Clear the project's category
    #[arg(long, conflicts_with = "set_category")]
    pub clear_category: bool,
}

pub fn run(args: ProjectArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let (category, success_message) = if let Some(ref cat) = args.set_category {
        (
            Some(cat.as_str()),
            format!("Set category '{}' for project '{}'", cat, args.name),
        )
    } else if args.clear_category {
        (
            None,
            format!("Cleared category for project '{}'", args.name),
        )
    } else {
        eprintln!("Error: specify --set-category <name> or --clear-category");
        std::process::exit(1);
    };

    match db::set_project_category(&conn, &args.name, category) {
        Ok(true) => println!("{}", success_message),
        Ok(false) => {
            eprintln!("Error: project '{}' not found", args.name);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    }
}
