import js from '@eslint/js';
import astro from 'eslint-plugin-astro';

export default [
  {
    ignores: [
      'archive/**',
      'dist/**',
      'engine/**',
      'node_modules/**',
      'public/legacy/**',
    ],
  },
  js.configs.recommended,
  ...astro.configs['flat/recommended'],
];
