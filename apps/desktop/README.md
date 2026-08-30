# Desktop Application

Native Rust desktop frontend for IL2CPP Explorer.

```bash
cargo run -p il2cpp-desktop
```

Built with `egui` + `eframe` and native `rfd` dialogs. No Tauri, WebView, Node.js, or JavaScript
runtime. The desktop app owns UI state only; IL2CPP parsing, C# rendering, addresses, and ARM64
disassembly remain in shared workspace crates.
