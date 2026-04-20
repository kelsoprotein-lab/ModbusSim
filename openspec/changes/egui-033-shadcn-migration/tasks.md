## 1. Phase 1 · egui 生态 bump 到 0.33

- [x] 1.1 workspace `Cargo.toml` 修改：`egui` `eframe` `egui_extras` 统一 `"0.33.3"`；`catppuccin-egui` 从 `features = ["egui29"]` 改 `["egui33"]` + 版本 `"5.7"`；`egui-phosphor` 从 `"0.7"` 改 `"0.11"`
- [x] 1.2 `git tag pre-egui-033` 打回退基线
- [x] 1.3 `cargo check --workspace 2>&1 | head -100` 收集所有编译错误
- [x] 1.4 按 compiler error 修 `install_cjk_fonts`（font backend 换 skrifa；可能需改 `Font::from_bytes` → `FontData::from_static` 等）
- [x] 1.5 修 `TextEdit::load_state` / `CCursorRange::two` 调用点（T6 搜索框全选逻辑，位于 `modbussim-egui/src/app.rs` RegisterGroup 分支）
- [x] 1.6 修 `TableBuilder.scroll_to_row` 如 API 有变
- [x] 1.7 修 eframe `NativeOptions.viewport` / entry point 如 API 有变（`main.rs` 两处）
- [x] 1.8 `cargo build --workspace` 干净通过（warnings 可保留，无 error）
- [x] 1.9 `cargo test --workspace --exclude modbussim-app --exclude modbusmaster-app` 全绿
- [x] 1.10 本地 `mbpoll` 冒烟：Slave 读写 / 搜索框跳转 / 抖动开关
- [x] 1.11 commit: `chore(egui): bump to 0.33.3 (egui/eframe/extras/phosphor/catppuccin)`

## 2. Phase 2 · 引入 egui-shadcn 并试跑

- [ ] 2.1 workspace `Cargo.toml` 加 `egui-shadcn = "0.3"`
- [ ] 2.2 `modbussim-ui-shared/Cargo.toml` 加 `egui-shadcn = { workspace = true }`
- [ ] 2.3 新建 `crates/modbussim-ui-shared/src/shadcn_preview.rs`，写一个 `preview_widgets(ui)` 函数展示 Button 的 4 个变体 + Switch + Size variants，用于手动验证
- [ ] 2.4 新建 bin target `crates/modbussim-egui/src/bin/shadcn_preview.rs`，单窗口调 `preview_widgets`
- [ ] 2.5 在 `modbussim-ui-shared/src/lib.rs` 加 `pub mod shadcn_preview;` 让 bin 能引
- [ ] 2.6 `cargo run -p modbussim-egui --bin shadcn_preview` 目视四种按钮 + 开关能画出来
- [ ] 2.7 阅读 egui-shadcn 文档 + 源码，在 design.md "未决问题" 里填答案：变体符号 / theme token API / size 枚举名
- [ ] 2.8 commit: `feat(ui-shared): 引入 egui-shadcn + preview bin 验证 API`

## 3. Phase 3 · ui.rs wrapper 切到 shadcn

- [ ] 3.1 在 `theme::apply` 里追加 shadcn theme token 同步（primary = #cc7832, destructive = #bc3f3c, background = L1, foreground = #d4d7db, ring = accent）
- [ ] 3.2 `primary_button` 内部替换为 `shadcn::button(ui, text).variant(Default).size(Md)`
- [ ] 3.3 `secondary_button` 内部替换为 Outline 变体
- [ ] 3.4 `danger_button` 内部替换为 Destructive 变体
- [ ] 3.5 `icon_button` 内部替换为 Ghost 变体 + icon content
- [ ] 3.6 `toggle_switch` 内部整段替换为 `shadcn::switch(ui, value)`，保留 `Response` 返回
- [ ] 3.7 旧自绘代码（`rect_filled` 画椭圆 / `circle_filled` 画滑块）从 ui.rs 中删除
- [ ] 3.8 `cargo build -p modbussim-egui -p modbusmaster-egui` 无错
- [ ] 3.9 启动 Slave 手测：批量添加按钮是橙 shadcn Default；"停止 / 删除"是 shadcn Outline；FC01 toggle 是 shadcn Switch（有滑动动画）
- [ ] 3.10 启动 Master 手测：三个 tab 按钮 OK；读/写/轮询按钮 OK；结果表 OK
- [ ] 3.11 commit: `feat(ui-shared): 按钮 + toggle 切换到 shadcn 底层实现`

## 4. Phase 4 · 清理 + CI

- [ ] 4.1 workspace `Cargo.toml` 删 `egui-modal` / `egui-toast` 两条依赖声明
- [ ] 4.2 `modbussim-ui-shared/Cargo.toml` / `modbussim-egui/Cargo.toml` / `modbusmaster-egui/Cargo.toml` 各自删两条引用
- [ ] 4.3 删 Phase 2 创建的 `shadcn_preview.rs` 源文件 + bin target + lib.rs 的 `pub mod`（清理 scaffolding）
- [ ] 4.4 `cargo build --release -p modbussim-egui -p modbusmaster-egui` 通过
- [ ] 4.5 全 workspace test 绿
- [ ] 4.6 `openspec-cn validate egui-033-shadcn-migration` 通过
- [ ] 4.7 `git push origin refactor/egui-skeleton`
- [ ] 4.8 `gh run list --branch refactor/egui-skeleton --limit 2` 观察 CI 三平台
- [ ] 4.9 如 CI 失败（ubuntu 缺 skrifa 系统库 / macOS 新依赖），修 `.github/workflows/ci-egui.yml` + 再 push
- [ ] 4.10 commit: `chore: 删除死依赖 egui-modal / egui-toast + 清理 shadcn_preview scaffolding`
