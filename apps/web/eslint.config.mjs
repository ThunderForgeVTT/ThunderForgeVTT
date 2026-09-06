import js from "@eslint/js";
import eslintConfigPrettier from "eslint-config-prettier";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "dist/**",
      "postcss.config.js",
      "scripts/**",
      "tailwind.config.js",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  eslintConfigPrettier,
  {
    // The service worker is not a browser page and does not have a `window`.
    // Linting it with the browser globals reported `self` and `caches` as
    // undefined — seven errors describing the environment wrongly rather
    // than the code. Its own globals are what it actually runs with.
    files: ["public/sw.js"],
    languageOptions: {
      ecmaVersion: "latest",
      globals: globals.serviceworker,
    },
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: "latest",
      globals: globals.browser,
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      // An underscore prefix means "deliberately unused" — a parameter kept
      // because the signature requires it, or a destructured field being
      // skipped. Without this the only way to satisfy the rule is to delete
      // the name, which loses the documentation of what that position *is*.
      // `_sellerActorId` in useGenieSession was already written this way, in
      // the expectation the convention was configured. It is now.
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
          destructuredArrayIgnorePattern: "^_",
        },
      ],

      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
    },
  },
  {
    files: ["src/engine/**/*.{ts,tsx}"],
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
    },
  },
  {
    files: ["src/pages/**/*.tsx"],
    rules: {
      "react-refresh/only-export-components": "off",
    },
  },
);
