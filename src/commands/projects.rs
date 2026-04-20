use clap::Args;
use comfy_table::modifiers::UTF8_SOLID_INNER_BORDERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use terminal_size::{terminal_size, Width};

use crate::config;
use crate::db;

#[derive(Args)]
pub struct ProjectsArgs {}

pub fn run(_args: ProjectsArgs) {
    let db_path = config::db_path();
    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Error: failed to write database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    let projects = match db::list_projects(&conn) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Error: failed to read database: {}", db_path.display());
            std::process::exit(1);
        }
    };

    if projects.is_empty() {
        println!("No projects.");
        return;
    }

    let term_width = terminal_size().map(|(Width(w), _)| w).unwrap_or(80);

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_SOLID_INNER_BORDERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(term_width);

    table.set_header(vec![
        Cell::new("Project").add_attribute(Attribute::Bold),
        Cell::new("Open").add_attribute(Attribute::Bold),
        Cell::new("Done").add_attribute(Attribute::Bold),
        Cell::new("Closed").add_attribute(Attribute::Bold),
        Cell::new("Total").add_attribute(Attribute::Bold),
    ]);

    for p in &projects {
        let total = p.open_count + p.done_count + p.closed_count;
        let open_cell = if p.open_count > 0 {
            Cell::new(p.open_count).fg(Color::Blue)
        } else {
            Cell::new(p.open_count).fg(Color::DarkGrey)
        };
        let done_cell = if p.done_count > 0 {
            Cell::new(p.done_count).fg(Color::Green)
        } else {
            Cell::new(p.done_count).fg(Color::DarkGrey)
        };
        let closed_cell = Cell::new(p.closed_count).fg(Color::DarkGrey);

        table.add_row(vec![
            Cell::new(&p.name).fg(Color::Cyan),
            open_cell,
            done_cell,
            closed_cell,
            Cell::new(total),
        ]);
    }

    for i in 1..=4 {
        if let Some(col) = table.column_mut(i) {
            col.set_cell_alignment(CellAlignment::Right);
        }
    }

    println!("{table}");
    println!();
    println!("{} projects", projects.len());
}
