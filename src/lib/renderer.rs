//! Rendering recipes to output formats.

use crate::recipe::*;
use std::fmt::Write;

/// Trait for rendering a Recipe to a string format.
pub trait Renderer {
    fn render(&self, recipe: &Recipe) -> String;
}

/// Renders a Recipe as conda-build meta.yaml (v0 format).
pub struct MetaYamlRenderer;

impl Renderer for MetaYamlRenderer {
    fn render(&self, recipe: &Recipe) -> String {
        let mut out = String::new();
        self.render_preamble(&mut out, &recipe.preamble);
        self.render_package(&mut out);
        self.render_source(&mut out, &recipe.source);
        self.render_build(&mut out, &recipe.build);
        self.render_requirements(&mut out, &recipe.requirements);
        self.render_test(&mut out, &recipe.test);
        self.render_about(&mut out, &recipe.about);
        self.render_extra(&mut out, &recipe.extra);
        out
    }
}

impl MetaYamlRenderer {
    fn render_preamble(&self, out: &mut String, preamble: &Preamble) {
        writeln!(out, "{{% set name = \"{}\" %}}", preamble.name).unwrap();
        writeln!(out, "{{% set version = \"{}\" %}}", preamble.version).unwrap();
        writeln!(out).unwrap();
    }

    fn render_package(&self, out: &mut String) {
        writeln!(out, "package:").unwrap();
        writeln!(out, "  name: {{{{ name }}}}").unwrap();
        writeln!(out, "  version: {{{{ version }}}}").unwrap();
        writeln!(out).unwrap();
    }

    fn render_source(&self, out: &mut String, source: &Source) {
        writeln!(out, "source:").unwrap();
        writeln!(out, "  url: {}", source.url).unwrap();
        writeln!(out, "  sha256: {}", source.sha256).unwrap();
        writeln!(out).unwrap();
    }

    fn render_build(&self, out: &mut String, build: &Build) {
        writeln!(out, "build:").unwrap();
        writeln!(out, "  number: 0").unwrap();
        if let Some(script) = &build.script {
            if script.contains('\n') {
                writeln!(out, "  script: |").unwrap();
                for line in script.lines() {
                    writeln!(out, "    {line}").unwrap();
                }
            } else {
                writeln!(out, "  script: {script}").unwrap();
            }
        }
        if build.with_run_exports {
            writeln!(out, "  run_exports:").unwrap();
            writeln!(
                out,
                "    - {{{{ pin_subpackage(\"{}\", max_pin=\"{}\") }}}}",
                build.name, build.max_pin
            )
            .unwrap();
        }
        writeln!(out).unwrap();
    }

    fn render_requirement(&self, out: &mut String, req: &Requirement) {
        match (&req.version, &req.selector) {
            (None, None) => writeln!(out, "    - {}", req.name).unwrap(),
            (Some(ver), None) => writeln!(out, "    - {} {}", req.name, ver).unwrap(),
            (None, Some(sel)) => writeln!(out, "    - {}  # [{sel}]", req.name).unwrap(),
            (Some(ver), Some(sel)) => {
                writeln!(out, "    - {} {}  # [{sel}]", req.name, ver).unwrap()
            }
        }
    }

    fn render_requirements(&self, out: &mut String, requirements: &Requirements) {
        writeln!(out, "requirements:").unwrap();
        if !requirements.build.is_empty() {
            writeln!(out, "  build:").unwrap();
            for req in &requirements.build {
                self.render_requirement(out, req);
            }
        }
        if !requirements.host.is_empty() {
            writeln!(out, "  host:").unwrap();
            for req in &requirements.host {
                self.render_requirement(out, req);
            }
        }
        if !requirements.run.is_empty() {
            writeln!(out, "  run:").unwrap();
            for req in &requirements.run {
                self.render_requirement(out, req);
            }
        }
        writeln!(out).unwrap();
    }

    fn render_test(&self, out: &mut String, test: &Test) {
        if test.commands.is_empty() {
            return;
        }
        writeln!(out, "test:").unwrap();
        writeln!(out, "  commands:").unwrap();
        for command in &test.commands {
            writeln!(out, "    - {command}").unwrap();
        }
        writeln!(out).unwrap();
    }

    fn render_about(&self, out: &mut String, about: &About) {
        writeln!(out, "about:").unwrap();
        if let Some(home) = &about.home {
            writeln!(out, "  home: {home}").unwrap();
        }
        if let Some(license) = &about.license {
            writeln!(out, "  license: {license}").unwrap();
        }
        if let Some(license_family) = &about.license_family {
            writeln!(out, "  license_family: {license_family}").unwrap();
        }
        match about.license_file.len() {
            0 => {}
            1 => writeln!(out, "  license_file: {}", about.license_file[0]).unwrap(),
            _ => {
                writeln!(out, "  license_file:").unwrap();
                for f in &about.license_file {
                    writeln!(out, "    - {f}").unwrap();
                }
            }
        }
        if let Some(summary) = &about.summary {
            writeln!(out, "  summary: {summary}").unwrap();
        }
        if let Some(dev_url) = &about.dev_url {
            writeln!(out, "  dev_url: {dev_url}").unwrap();
        }
        if let Some(doc_url) = &about.doc_url {
            writeln!(out, "  doc_url: {doc_url}").unwrap();
        }
        writeln!(out).unwrap();
    }

    fn render_extra(&self, out: &mut String, extra: &Extra) {
        if extra.additional_platforms.is_empty()
            && extra.recipe_maintainers.is_empty()
            && extra.identifiers.is_empty()
            && extra.skip_platforms.is_empty()
        {
            return;
        }
        writeln!(out, "extra:").unwrap();
        if !extra.additional_platforms.is_empty() {
            writeln!(out, "  additional-platforms:").unwrap();
            for plat in &extra.additional_platforms {
                writeln!(out, "    - {plat}").unwrap();
            }
        }
        if !extra.recipe_maintainers.is_empty() {
            writeln!(out, "  recipe-maintainers:").unwrap();
            for m in &extra.recipe_maintainers {
                writeln!(out, "    - {m}").unwrap();
            }
        }
        if !extra.identifiers.is_empty() {
            writeln!(out, "  identifiers:").unwrap();
            for id in &extra.identifiers {
                writeln!(out, "    - {id}").unwrap();
            }
        }
        if !extra.skip_platforms.is_empty() {
            writeln!(out, "  skip-platforms:").unwrap();
            for plat in &extra.skip_platforms {
                writeln!(out, "    - {plat}").unwrap();
            }
        }
        writeln!(out).unwrap();
    }
}
