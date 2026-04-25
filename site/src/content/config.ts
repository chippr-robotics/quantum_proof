import { defineCollection, z } from 'astro:content';

const docs = defineCollection({
  type: 'content',
  schema: z.object({
    title: z.string(),
    description: z.string().default(''),
    category: z.string().default('Other'),
    order: z.number().default(99),
    sourceFile: z.string().optional(),
  }),
});

export const collections = { docs };
