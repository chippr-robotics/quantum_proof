import { defineConfig } from 'astro/config';
import react from '@astrojs/react';
import tailwind from '@astrojs/tailwind';
import mdx from '@astrojs/mdx';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';

// The site is served from a custom domain (quantum.chipprbots.com) when DNS is
// configured. Set PUBLIC_BASE=/quantum_proof to build for the GitHub Pages
// project URL fallback (https://chippr-robotics.github.io/quantum_proof/).
const base = process.env.PUBLIC_BASE ?? '/';
const site =
  process.env.PUBLIC_SITE ??
  (base === '/' ? 'https://quantum.chipprbots.com' : 'https://chippr-robotics.github.io');

export default defineConfig({
  site,
  base,
  trailingSlash: 'ignore',
  integrations: [
    react(),
    tailwind({ applyBaseStyles: false }),
    mdx(),
  ],
  markdown: {
    remarkPlugins: [remarkMath],
    rehypePlugins: [[rehypeKatex, { strict: false }]],
    shikiConfig: {
      themes: { light: 'github-light', dark: 'github-dark' },
      wrap: true,
    },
  },
  vite: {
    ssr: {
      noExternal: ['katex'],
    },
  },
});
