//! Project scaffolding command
//!
//! This is intentionally simple and template-driven so new starter templates can
//! be added by extending the in-crate registry.

use std::path::{Path, PathBuf};

use clap::Args;
use console::style;
use dialoguer::{Confirm, Input};
use tracing::info;

use crate::cli::Cli;

#[derive(Debug, Args)]
pub struct ScaffoldCommand {
    /// Template to scaffold (astro, marketing, expo, hono)
    pub template: Option<String>,

    /// Project name
    #[arg(short, long)]
    pub name: Option<String>,

    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Create without prompts
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Overwrite existing files
    #[arg(short, long)]
    pub force: bool,

    /// Show generated file list only
    #[arg(long)]
    pub dry_run: bool,

    /// List available templates and aliases
    #[arg(long)]
    pub list: bool,
}

#[derive(Debug)]
struct TemplateFile {
    path: &'static str,
    contents: &'static str,
}

#[derive(Debug)]
struct ProjectTemplate {
    id: &'static str,
    aliases: &'static [&'static str],
    name: &'static str,
    description: &'static str,
    files: &'static [TemplateFile],
}

#[derive(Debug)]
struct TemplateContext {
    name: String,
    slug: String,
    title: String,
}

impl ProjectTemplate {
    fn matches(&self, value: &str) -> bool {
        let value = value.to_ascii_lowercase();
        self.id == value || self.aliases.iter().any(|a| a == &value)
    }

    fn apply_template(&self, template: &str, ctx: &TemplateContext) -> String {
        let replacements = [
            ("{{name}}", ctx.name.as_str()),
            ("{{slug}}", ctx.slug.as_str()),
            ("{{title}}", ctx.title.as_str()),
        ];
        let mut rendered = template.to_string();
        for (key, value) in replacements {
            rendered = rendered.replace(key, value);
        }
        rendered
    }
}

fn templates() -> &'static [ProjectTemplate] {
    const ASTRO_FILES: &[TemplateFile] = &[
        TemplateFile {
            path: "package.json",
            contents: r#"{
  "name": "{{slug}}",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "astro dev",
    "build": "astro build",
    "preview": "astro preview"
  },
  "dependencies": {
    "astro": "^4.15.0"
  }
}
"#,
        },
        TemplateFile {
            path: ".gitignore",
            contents: r#"node_modules
dist
.astro

"#,
        },
        TemplateFile {
            path: "astro.config.mjs",
            contents: r#"import { defineConfig } from "astro/config";

export default defineConfig({});
"#,
        },
        TemplateFile {
            path: "src/pages/index.astro",
            contents: r#"---
const features = ["Quick pages", "Markdown content", "Simple deploys"];
---

<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{title}}</title>
  </head>
  <body>
    <main>
      <h1>{{title}}</h1>
      <p>
        Welcome to {{title}}. This is a starting point for a content-first Astro
        website.
      </p>
      <ul>
        {features.map((feature) => <li>{feature}</li>)}
      </ul>
    </main>
  </body>
</html>
"#,
        },
    ];

    const MARKETING_FILES: &[TemplateFile] = &[
        TemplateFile {
            path: "package.json",
            contents: r#"{
  "name": "{{slug}}",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  },
  "devDependencies": {
    "vite": "^5.2.0"
  }
}
"#,
        },
        TemplateFile {
            path: ".gitignore",
            contents: r#"node_modules
dist

"#,
        },
        TemplateFile {
            path: "index.html",
            contents: r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{title}}</title>
  </head>
  <body>
    <main id="app"></main>
    <script type="module" src="/src/main.js"></script>
  </body>
</html>
"#,
        },
        TemplateFile {
            path: "src/main.js",
            contents: r#"import "./styles.css";

document.body.innerHTML = `
  <main>
    <h1>{{title}}</h1>
    <p>Launchpad for your marketing site. Replace with your brand content.</p>
    <section>
      <h2>Features</h2>
      <ul>
        <li>Simple landing page</li>
        <li>Fast local dev with Vite</li>
        <li>Easy deployment to Netlify, Vercel, or Cloudflare</li>
      </ul>
    </section>
  </main>
`;
"#,
        },
        TemplateFile {
            path: "src/styles.css",
            contents: r#":root {
  font-family: "Inter", "Segoe UI", sans-serif;
  color: #111827;
  background: #f8fafc;
}

body {
  margin: 0;
  padding: 3rem;
}

main {
  max-width: 720px;
  margin: 0 auto;
}
"#,
        },
    ];

    const EXPO_FILES: &[TemplateFile] = &[
        TemplateFile {
            path: "package.json",
            contents: r#"{
  "name": "{{slug}}",
  "private": true,
  "main": "expo/AppEntry.js",
  "scripts": {
    "start": "expo start",
    "android": "expo run:android",
    "ios": "expo run:ios",
    "web": "expo start --web"
  },
  "dependencies": {
    "expo": "^51.0.0",
    "expo-status-bar": "~1.12.1",
    "react": "18.2.0",
    "react-native": "0.74.5"
  },
  "devDependencies": {
    "typescript": "^5.4.0"
  }
}
"#,
        },
        TemplateFile {
            path: "app.json",
            contents: r##"{
  "expo": {
    "name": "{{name}}",
    "slug": "{{slug}}",
    "scheme": "{{slug}}",
    "version": "1.0.0",
    "orientation": "portrait",
    "platforms": ["ios", "android", "web"],
    "ios": {
      "supportsTablet": true
    },
    "android": {
      "adaptiveIcon": {
        "backgroundColor": "#ffffff"
      }
    },
    "web": {
      "bundler": "metro"
    }
  }
}
"##,
        },
        TemplateFile {
            path: "App.tsx",
            contents: r##"import { StatusBar } from "expo-status-bar";
import { StyleSheet, Text, View } from "react-native";

export default function App() {
  return (
    <View style={styles.container}>
      <Text style={styles.title}>{{title}}</Text>
      <Text>Welcome to your new Expo app.</Text>
      <StatusBar style="auto" />
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    backgroundColor: "#fff",
  },
  title: {
    fontSize: 24,
    fontWeight: "600",
  },
});
"##,
        },
        TemplateFile {
            path: "babel.config.js",
            contents: r#"module.exports = function(api) {
  api.cache(true);
  return {
    presets: ["babel-preset-expo"],
  };
};
"#,
        },
    ];

    const HONO_FILES: &[TemplateFile] = &[
        TemplateFile {
            path: "package.json",
            contents: r#"{
  "name": "{{slug}}",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "tsx watch src/index.ts",
    "start": "node --loader tsx src/index.ts"
  },
  "dependencies": {
    "hono": "^4.5.0"
  },
  "devDependencies": {
    "tsx": "^4.16.0",
    "typescript": "^5.4.0"
  }
}
"#,
        },
        TemplateFile {
            path: ".gitignore",
            contents: r#"node_modules
dist

"#,
        },
        TemplateFile {
            path: "src/index.ts",
            contents: r#"import { Hono } from "hono";

const app = new Hono();

app.get("/", (c) => c.text("Hello from {{name}}"));
app.get("/health", (c) => c.json({ status: "ok" }));

export default app;
export type HonoApp = typeof app;
"#,
        },
        TemplateFile {
            path: "tsconfig.json",
            contents: r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "dist"
  },
  "include": ["src/**/*"]
}
"#,
        },
    ];

    const TEMPLATES: &[ProjectTemplate] = &[
        ProjectTemplate {
            id: "astro",
            aliases: &["astro", "astro-site", "astro-web"],
            name: "Astro Website",
            description: "Content-driven Astro site for docs, blogs, and marketing pages.",
            files: ASTRO_FILES,
        },
        ProjectTemplate {
            id: "marketing",
            aliases: &["marketing", "marketing-site"],
            name: "Marketing Website",
            description:
                "Simple lightweight marketing site with Vite and a starter landing page structure.",
            files: MARKETING_FILES,
        },
        ProjectTemplate {
            id: "expo",
            aliases: &["expo", "expo-app", "react-native"],
            name: "Expo App",
            description: "Expo app with a basic React Native entry screen.",
            files: EXPO_FILES,
        },
        ProjectTemplate {
            id: "hono",
            aliases: &["hono", "hono-api", "api"],
            name: "Hono API",
            description: "Small Hono API scaffold with health and root routes.",
            files: HONO_FILES,
        },
    ];

    TEMPLATES
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars().map(|c| c.to_ascii_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if (ch == '-' || ch == '_' || ch.is_whitespace()) && !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn title_from_name(name: &str) -> String {
    let title = name
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut segment_str = String::new();
                    segment_str.push(first.to_ascii_uppercase());
                    segment_str.push_str(chars.as_str());
                    segment_str
                }
                None => String::new(),
            }
        })
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if title.is_empty() {
        "Project".to_string()
    } else {
        title
    }
}

fn resolve_name(
    explicit: Option<&str>,
    output: Option<&Path>,
    yes: bool,
) -> anyhow::Result<String> {
    if let Some(value) = explicit {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Project name cannot be empty");
        }
        return Ok(trimmed.to_string());
    }

    if let Some(output) = output {
        if let Some(file_name) = output.file_name().and_then(|name| name.to_str()) {
            if !file_name.is_empty() && file_name != "." {
                return Ok(file_name.to_string());
            }
        }
    }

    if yes {
        return Ok("my-project".to_string());
    }

    Input::<String>::new()
        .with_prompt("Project name")
        .default("my-project".to_string())
        .interact_text()
        .map_err(Into::into)
}

fn resolve_output_dir(cwd: &Path, output: Option<&PathBuf>, name: &str) -> PathBuf {
    output
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
        })
        .unwrap_or_else(|| cwd.join(name))
}

fn ensure_template<'a>(
    name: &str,
    list: &'a [ProjectTemplate],
) -> anyhow::Result<&'a ProjectTemplate> {
    list.iter()
        .find(|template| template.matches(name))
        .ok_or_else(|| {
            let available = list
                .iter()
                .map(|template| template.id)
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::anyhow!("Unknown template: {name}. Available templates: {available}",)
        })
}

impl ScaffoldCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(
            template = self.template.as_deref().unwrap_or(""),
            dry_run = self.dry_run,
            "executing scaffold command"
        );

        if self.list {
            if !cli.quiet {
                println!("{}", style("Available project templates:").bold());
                for template in templates() {
                    println!(
                        "  {:<12} [{}] {}",
                        style(template.id).green(),
                        template.aliases.join(", "),
                        template.description
                    );
                }
            }
            return Ok(());
        }

        let template_name = self.template.as_deref().ok_or_else(|| {
            anyhow::anyhow!("Missing template. Use --list to see available scaffolds.")
        })?;

        let selected = ensure_template(template_name, templates())?;
        let cwd = std::env::current_dir()?;
        let project_name = resolve_name(self.name.as_deref(), self.output.as_deref(), self.yes)?;
        let slug = slugify(&project_name);
        if slug.is_empty() {
            anyhow::bail!("Project name must contain at least one alphanumeric character");
        }

        let output_dir = resolve_output_dir(&cwd, self.output.as_ref(), &slug);

        if output_dir.exists() && !self.force {
            if self.yes {
                anyhow::bail!(
                    "Output directory already exists: {}. Use --force to overwrite.",
                    output_dir.display()
                );
            }

            let overwrite = Confirm::new()
                .with_prompt(format!(
                    "Output directory {} already exists. Overwrite existing files?",
                    output_dir.display()
                ))
                .default(false)
                .interact()?;
            if !overwrite {
                println!("{}", style("Aborted.").yellow());
                return Ok(());
            }
        }

        let project_title = title_from_name(&project_name);
        let ctx = TemplateContext {
            name: project_name,
            slug: slug.clone(),
            title: project_title,
        };

        let target = output_dir;
        std::fs::create_dir_all(&target)?;

        if !cli.quiet {
            println!(
                "{} Scaffolding {} into {}",
                style("→").blue(),
                style(selected.name).cyan(),
                style(target.display()).cyan()
            );
        }

        let mut created_files = Vec::new();
        for file in selected.files {
            let path = target.join(file.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = selected.apply_template(file.contents, &ctx);
            created_files.push(path.clone());
            if self.dry_run {
                continue;
            }
            std::fs::write(&path, content)?;
        }

        if self.dry_run {
            if !cli.quiet {
                println!("{}", style("Would create files:").bold());
                for path in created_files {
                    println!(
                        "  {}",
                        style(path.strip_prefix(&target).unwrap_or(&path).display()).dim()
                    );
                }
            }
            return Ok(());
        }

        if !cli.quiet {
            println!("{}", style("Created files:").bold());
            for path in created_files {
                println!("  ✓ {}", style(path.display()).green());
            }
            println!();
            println!("{} Next steps:", style("→").blue());
            println!("  cd {}", style(target.display()).cyan());
            println!("  npm install");
            println!("  npm run dev");
        }

        Ok(())
    }
}
