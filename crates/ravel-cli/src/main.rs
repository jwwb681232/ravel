mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ravel_generator::Generator;

#[derive(Parser)]
#[command(name = "ravel")]
#[command(version)]
#[command(about = "Laravel-inspired Rust Framework CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Ravel project
    New {
        /// Project name (CamelCase)
        name: String,
    },

    /// Generate scaffolding files
    #[command(subcommand)]
    Make(MakeCommands),

    /// Start the development server
    Serve,

    /// List all registered routes
    #[command(name = "route:list")]
    RouteList,

    /// Database subcommands
    #[command(subcommand)]
    Db(DbCommands),

    /// Run pending migrations
    #[command(name = "migrate")]
    Migrate {
        /// Number of migrations to run (default: all)
        #[arg(short, long)]
        steps: Option<u32>,
    },

    /// Rollback migrations
    #[command(name = "migrate:rollback")]
    MigrateRollback {
        /// Number of migrations to rollback (default: 1)
        #[arg(short, long)]
        steps: Option<u32>,
    },

    /// Refresh (rollback all + re-apply)
    #[command(name = "migrate:refresh")]
    MigrateRefresh,

    /// Fresh (drop all tables + re-apply)
    #[command(name = "migrate:fresh")]
    MigrateFresh,

    /// Show migration status
    #[command(name = "migrate:status")]
    MigrateStatus,

    /// Generate an application key
    #[command(name = "key:generate")]
    KeyGenerate,
}

#[derive(Subcommand)]
enum MakeCommands {
    /// Create a new Controller
    Controller { name: String },
    /// Create a new Middleware
    Middleware { name: String },
    /// Create a new Model (Ravel Eloquent entity)
    Model {
        name: String,
        /// Also generate a migration for this model
        #[arg(short = 'm', long)]
        migration: bool,
    },
    /// Create a new Migration (auto-timestamped)
    Migration { name: String },
    /// Create a new Seeder
    Seeder { name: String },
    /// Create a new ServiceProvider
    Provider { name: String },
    /// Create a new FormRequest
    Request { name: String },
    /// Create a new Job
    Job { name: String },
}

#[derive(Subcommand)]
enum DbCommands {
    /// Run database seeders
    Seed,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => {
            commands::new::handle(&name)?;
        }

        Commands::Make(cmd) => {
            let g = Generator::new(".");

            match cmd {
                MakeCommands::Controller { name } => {
                    g.scaffold_controller(&name)?;
                    println!("✅ Controller created: {}", name);
                }
                MakeCommands::Middleware { name } => {
                    g.scaffold_middleware(&name)?;
                    println!("✅ Middleware created: {}", name);
                }
                MakeCommands::Model { name, migration } => {
                    g.scaffold_model(&name)?;
                    println!("✅ Model created: {}", name);
                    if migration {
                        g.scaffold_migration(&format!("Create{name}"))?;
                        println!("✅ Migration created for: Create{}", name);
                    }
                }
                MakeCommands::Migration { name } => {
                    g.scaffold_migration(&name)?;
                    println!("✅ Migration created for: {}", name);
                }
                MakeCommands::Seeder { name } => {
                    g.scaffold_seeder(&name)?;
                    println!("✅ Seeder created: {}", name);
                }
                MakeCommands::Provider { name } => {
                    g.scaffold_provider(&name)?;
                    println!("✅ Provider created: {}", name);
                }
                MakeCommands::Request { name } => {
                    g.scaffold_request(&name)?;
                    println!("✅ FormRequest created: {}", name);
                }
                MakeCommands::Job { name } => {
                    g.ensure_dir("app/Jobs")?;
                    g.scaffold_job(&name)?;
                    println!("✅ Job created: {}", name);
                }
            }
        }

        Commands::Serve => {
            commands::serve::handle()?;
        }

        Commands::RouteList => {
            commands::route_list::handle()?;
        }

        Commands::Db(cmd) => match cmd {
            DbCommands::Seed => {
                commands::db_seed::handle()?;
            }
        },

        Commands::Migrate { steps } => {
            commands::migrate::handle_migrate(steps).await?;
        }

        Commands::MigrateRollback { steps } => {
            commands::migrate::handle_rollback(steps).await?;
        }

        Commands::MigrateRefresh => {
            commands::migrate::handle_refresh().await?;
        }

        Commands::MigrateFresh => {
            commands::migrate::handle_fresh().await?;
        }

        Commands::MigrateStatus => {
            commands::migrate::handle_status().await?;
        }

        Commands::KeyGenerate => {
            commands::key_generate::handle()?;
        }
    }

    Ok(())
}
