#!/usr/bin/env node
// Copies oathbreaker/docs/*.md into site/src/content/docs/ and injects
// frontmatter so they can be rendered by Astro's content collection.
//
// The source of truth is oathbreaker/docs/. Engineers edit those files.
// This script keeps the site copies in sync at build time.

import { promises as fs } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const SITE_ROOT = path.resolve(__dirname, '..');
const REPO_ROOT = path.resolve(SITE_ROOT, '..');
const SOURCE_DIR = path.join(REPO_ROOT, 'oathbreaker', 'docs');
const DEST_DIR = path.join(SITE_ROOT, 'src', 'content', 'docs');

// Ordering + display titles + descriptions for each doc. Files not listed
// here are still copied with auto-derived titles, but appear at the end.
const DOC_META = {
  'ARCHITECTURE.md': {
    order: 1,
    title: 'Architecture',
    description: 'Core design patterns and how the seven crates fit together.',
    category: 'Foundations',
  },
  'CIRCUIT_ARCHITECTURE.md': {
    order: 2,
    title: 'Circuit Architecture',
    description: 'Gate-level design, the [a]G + [b]Q group-action map, and optimization strategy.',
    category: 'Foundations',
  },
  'OATH_TIERS.md': {
    order: 3,
    title: 'Oath Tiers',
    description: 'Curve parameters and tier-by-tier benchmark results.',
    category: 'Foundations',
  },
  'BENCHMARKING.md': {
    order: 4,
    title: 'Benchmarking',
    description: 'Methodology for resource counting and scaling projections.',
    category: 'Results',
  },
  'V3_OPTIMIZATIONS.md': {
    order: 5,
    title: 'V3 Optimizations',
    description: 'Karatsuba, symmetry squaring, Binary GCD — the optimization history.',
    category: 'Results',
  },
  'COMPARISON.md': {
    order: 6,
    title: 'Prior Work Comparison',
    description: 'Where Oathbreaker sits relative to Roetteler, Litinski, and Google.',
    category: 'Results',
  },
  'ZKP_GUIDE.md': {
    order: 7,
    title: 'ZK Proof Guide',
    description: 'The SP1 zkVM → Groth16 SNARK pipeline.',
    category: 'Verification',
  },
  'VERIFICATION.md': {
    order: 8,
    title: 'Verification',
    description: 'Soundness analysis and classical ground-truth checks.',
    category: 'Verification',
  },
  'NISQ_ROADMAP.md': {
    order: 9,
    title: 'NISQ Roadmap',
    description: 'POC vs. real-path constraints on the Qiskit/IBM execution stack.',
    category: 'Hardware',
  },
  'LIMITATIONS.md': {
    order: 10,
    title: 'Limitations',
    description: 'What this project is — and is not.',
    category: 'Hardware',
  },
};

function slugify(filename) {
  return filename
    .replace(/\.md$/, '')
    .toLowerCase()
    .replace(/[_]/g, '-');
}

function escapeYaml(value) {
  if (typeof value === 'number') return String(value);
  return `"${String(value).replace(/"/g, '\\"')}"`;
}

function buildFrontmatter(meta) {
  const lines = ['---'];
  for (const [key, value] of Object.entries(meta)) {
    lines.push(`${key}: ${escapeYaml(value)}`);
  }
  lines.push('---', '');
  return lines.join('\n');
}

async function main() {
  await fs.mkdir(DEST_DIR, { recursive: true });

  // Clean previously-synced markdown files (preserve .gitkeep).
  const existing = await fs.readdir(DEST_DIR).catch(() => []);
  for (const f of existing) {
    if (f.endsWith('.md')) {
      await fs.unlink(path.join(DEST_DIR, f));
    }
  }

  let entries;
  try {
    entries = await fs.readdir(SOURCE_DIR, { withFileTypes: true });
  } catch (err) {
    console.error(`[sync-docs] could not read ${SOURCE_DIR}: ${err.message}`);
    process.exit(1);
  }

  const files = entries
    .filter((e) => e.isFile() && e.name.endsWith('.md'))
    .map((e) => e.name);

  let synced = 0;
  for (const filename of files) {
    const meta = DOC_META[filename] ?? {};
    const slug = slugify(filename);
    const order = meta.order ?? 99;
    const title =
      meta.title ??
      filename
        .replace(/\.md$/, '')
        .split(/[_-]/)
        .map((w) => w.charAt(0) + w.slice(1).toLowerCase())
        .join(' ');
    const description = meta.description ?? '';
    const category = meta.category ?? 'Other';

    const sourcePath = path.join(SOURCE_DIR, filename);
    const destPath = path.join(DEST_DIR, `${slug}.md`);

    let body = await fs.readFile(sourcePath, 'utf8');

    // Strip an existing top-level "# Title" heading if present — the layout
    // renders the title from frontmatter.
    body = body.replace(/^#\s+.+\n+/, '');

    // Rewrite relative links to other docs so they resolve under /docs/<slug>.
    body = body.replace(/\]\(\.\/([A-Z0-9_-]+)\.md\)/g, (_m, name) => {
      return `](/docs/${slugify(name + '.md')})`;
    });
    body = body.replace(/\]\(([A-Z0-9_-]+)\.md\)/g, (_m, name) => {
      return `](/docs/${slugify(name + '.md')})`;
    });

    const frontmatter = buildFrontmatter({
      title,
      description,
      category,
      order,
      sourceFile: `oathbreaker/docs/${filename}`,
    });

    await fs.writeFile(destPath, frontmatter + body, 'utf8');
    synced += 1;
  }

  console.log(`[sync-docs] synced ${synced} doc(s) → ${path.relative(REPO_ROOT, DEST_DIR)}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
