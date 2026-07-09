export default {
  title: "Saikuro",
  url: "https://nisoku.org/Saikuro",
  logo: { alt: "Saikuro", href: "./" },
  favicon: "",
  theme: {
    name: "ruby",
    defaultMode: "system",
    enableModeToggle: true,
    positionMode: "top",
    codeHighlight: true,
    customCss: [],
    copyWidgets: {
      enabled: true,
      raw: true,
      context: true,
    },
  },
  layout: {
    footer: {
      style: "complete",
      description: "Cross-language invocation fabric for multi-runtime integration.",
      branding: true,
      columns: [
        {
          title: "Resources",
          links: [
            { text: "Getting Started", url: "./getting-started/quickstart" },
            { text: "Invocation Primitives", url: "./guide/invocations" },
            { text: "Protocol Reference", url: "./api/" },
          ],
        },
        {
          title: "Community",
          links: [
            { text: "GitHub", url: "https://github.com/Nisoku/Saikuro" },
            { text: "Issues", url: "https://github.com/Nisoku/Saikuro/issues" },
            { text: "Discussions", url: "https://github.com/Nisoku/Saikuro/discussions" },
          ],
        },
      ],
    },
  },
  plugins: {
    search: {
      semantic: true,
      showConfidence: true,
    },
    seo: {
      defaultDescription:
        "Saikuro is a cross-language invocation fabric. Seamless function calls, streams, and channels across TypeScript, Python, C#, Rust, C, and C++.",
      openGraph: { defaultImage: "" },
      twitter: { cardType: "summary_large_image" },
    },
    sitemap: {
      defaultChangefreq: "weekly",
      defaultPriority: 0.8,
    },
    mermaid: {},
    git: {},
    llms: {
      fullContext: true,
    },
  },
  search: true,
  minify: true,
  autoTitleFromH1: true,
  copyCode: true,
  pageNavigation: true,
  navigation: [
    { title: "Home", path: "/", icon: "home" },
    {
      title: "Getting Started",
      icon: "rocket",
      collapsible: false,
      children: [
        { title: "Quick Start", path: "/getting-started/quickstart", icon: "play" },
        { title: "Installation", path: "/getting-started/installation", icon: "download" },
        { title: "Core Concepts", path: "/getting-started/concepts", icon: "book" },
      ],
    },
    {
      title: "Guide",
      icon: "book-open",
      collapsible: false,
      children: [
        { title: "Invocation Primitives", path: "/guide/invocations", icon: "zap" },
        { title: "Schema", path: "/guide/schema", icon: "file-text" },
        { title: "Code Generation", path: "/guide/codegen", icon: "cpu" },
        { title: "Transports", path: "/guide/transports", icon: "radio" },
        { title: "Storage", path: "/guide/storage", icon: "database" },
        { title: "Error Handling", path: "/guide/errors", icon: "logs" },
        { title: "Logging", path: "/guide/logging", icon: "terminal" },
        { title: "WASM", path: "/guide/wasm", icon: "globe" },
        { title: "Commands", path: "/guide/commands", icon: "terminal" },
        { title: "Examples", path: "/guide/examples", icon: "terminal" },
      ],
    },
    {
      title: "Adapters",
      icon: "code",
      path: "/adapters/",
      children: [
        {
          title: "TypeScript", path: "/adapters/typescript/", icon: "file-code",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/typescript/api-reference", icon: "file-code" },
            { title: "Examples", path: "/adapters/typescript/examples", icon: "terminal" },
          ],
        },
        {
          title: "Python", path: "/adapters/python/", icon: "code",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/python/api-reference", icon: "file-code" },
            { title: "Examples", path: "/adapters/python/examples", icon: "terminal" },
          ],
        },
        {
          title: "Rust", path: "/adapters/rust/", icon: "box",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/rust/api-reference", icon: "file-code" },
            { title: "Examples", path: "/adapters/rust/examples", icon: "terminal" },
          ],
        },
        {
          title: "C#", path: "/adapters/csharp/", icon: "hash",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/csharp/api-reference", icon: "file-code" },
            { title: "Examples", path: "/adapters/csharp/examples", icon: "terminal" },
          ],
        },
        {
          title: "C", path: "/adapters/c/", icon: "terminal",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/c/api-reference", icon: "file-code" },
            { title: "Examples", path: "/adapters/c/examples", icon: "terminal" },
          ],
        },
        {
          title: "C++", path: "/adapters/cpp/", icon: "cpu",
          collapsible: true,
          children: [
            { title: "API Reference", path: "/adapters/cpp/api-reference", icon: "file-code" },
            { title: "Examples", path: "/adapters/cpp/examples", icon: "terminal" },
          ],
        },
      ],
    },
    {
      title: "Core Reference",
      icon: "file-code",
      collapsible: false,
      children: [
        { title: "Protocol & Runtime", path: "/api/", icon: "box" },
        { title: "CLI Reference", path: "/api/cli", icon: "terminal" },
      ],
    },
    {
      title: "GitHub",
      path: "https://github.com/Nisoku/Saikuro",
      icon: "github",
      external: true,
    },
  ],
  footer: "Built with [docmd](https://docmd.io). [View on GitHub](https://github.com/Nisoku/Saikuro).",
  editLink: {
    enabled: true,
    baseUrl: "https://github.com/Nisoku/Saikuro/edit/main/",
    text: "Edit this page",
  },
};
