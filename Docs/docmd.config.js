// docmd.config.js
module.exports = {
  // Core Metadata
  siteTitle: "Saikuro",
  siteUrl: "https://nisoku.github.io/Saikuro/docs",

  // Branding
  logo: {
    alt: "Saikuro",
    href: "./",
  },
  favicon: "",

  // Source & Output
  srcDir: "docs",
  outputDir: "site",

  // Theme & Layout
  theme: {
    name: "ruby",
    defaultMode: "system",
    enableModeToggle: true,
    positionMode: "top",
    codeHighlight: true,
    customCss: [],
  },

  // Features
  search: true,
  minify: true,
  autoTitleFromH1: true,
  copyCode: true,
  pageNavigation: true,

  // Navigation (Sidebar)
  navigation: [
    { title: "Home", path: "/", icon: "home" },
    {
      title: "Getting Started",
      icon: "rocket",
      collapsible: false,
      children: [
        {
          title: "Quick Start",
          path: "/getting-started/quickstart",
          icon: "play",
        },
        {
          title: "Core Concepts",
          path: "/getting-started/concepts",
          icon: "book",
        },
      ],
    },
    {
      title: "Guide",
      icon: "book-open",
      collapsible: false,
      children: [
        {
          title: "Invocation Primitives",
          path: "/guide/invocations",
          icon: "zap",
        },
        { title: "Schema", path: "/guide/schema", icon: "file-text" },
        { title: "Code Generation", path: "/guide/codegen", icon: "cpu" },
        { title: "Transports", path: "/guide/transports", icon: "radio" },
        { title: "Examples", path: "/guide/examples", icon: "terminal" },
      ],
    },
    {
      title: "Adapters",
      icon: "code",
      path: "/adapters/",
      children: [
        {
          title: "C",
          path: "/adapters/c/",
          icon: "terminal",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/c/api-reference/", icon: "file-code" },
            { title: "Examples", path: "/adapters/c/examples/", icon: "terminal" },
          ],
        },
        {
          title: "C++",
          path: "/adapters/cpp/",
          icon: "cpu",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/cpp/api-reference/", icon: "file-code" },
            { title: "Examples", path: "/adapters/cpp/examples/", icon: "terminal" },
          ],
        },
        {
          title: "C#",
          path: "/adapters/csharp/",
          icon: "hash",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/csharp/api-reference/", icon: "file-code" },
            { title: "Examples", path: "/adapters/csharp/examples/", icon: "terminal" },
          ],
        },
        {
          title: "Python",
          path: "/adapters/python/",
          icon: "code",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/python/api-reference/", icon: "file-code" },
            { title: "Examples", path: "/adapters/python/examples/", icon: "terminal" },
          ],

        },
        {
          title: "Rust",
          path: "/adapters/rust/",
          icon: "box",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/rust/api-reference/", icon: "file-code" },
            { title: "Examples", path: "/adapters/rust/examples/", icon: "terminal" },
          ],

        },
        {
          title: "TypeScript",
          path: "/adapters/typescript/",
          icon: "file-code",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/typescript/api-reference/", icon: "file-code" },
            { title: "Examples", path: "/adapters/typescript/examples/", icon: "terminal" },
          ],
        },
      ],
    },
    {
      title: "Core Reference",
      icon: "file-code",
      collapsible: false,
      children: [{ title: "Protocol & Runtime", path: "/api/", icon: "box" }],
    },
    {
      title: "GitHub",
      path: "https://github.com/Nisoku/Saikuro",
      icon: "github",
      external: true,
    },
  ],

  // Plugins
  plugins: {
    seo: {
      defaultDescription:
        "Cross-language invocation fabric. Seamless function calls, streams, and channels across TypeScript, Python, C#, Rust, and more.",
      openGraph: {
        defaultImage: "",
      },
      twitter: {
        cardType: "summary_large_image",
      },
    },
    sitemap: {
      defaultChangefreq: "weekly",
      defaultPriority: 0.8,
    },
  },

  // Footer
  footer:
    "Built with [docmd](https://docmd.io). [View on GitHub](https://github.com/Nisoku/Saikuro).",

  // Edit Link
  editLink: {
    enabled: true,
    baseUrl: "https://github.com/Nisoku/Saikuro/edit/main/Docs/docs",
    text: "Edit this page",
  },
};
