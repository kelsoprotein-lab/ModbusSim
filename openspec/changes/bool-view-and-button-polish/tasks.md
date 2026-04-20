## 1. ui.rs 新原语 + 按钮 fill 调整

- [x] 1.1 `crates/modbussim-ui-shared/src/ui.rs` 新增 `pub fn toggle_switch(ui, flavor, value: &mut bool) -> egui::Response`（40×18 椭圆轨道 + 14/16 px 白滑块；on=success 绿、off=bg_hover 灰；hover 滑块放大到 16 px；整区域 Sense::click）
- [x] 1.2 `secondary_button` 的 `.fill(Color32::TRANSPARENT)` 改为 `.fill(theme::bg_of(flavor, Layer::L2))`
- [x] 1.3 `cargo build -p modbussim-ui-shared` 通过
- [x] 1.4 commit: `feat(ui-shared): toggle_switch 椭圆开关 + secondary_button 淡 fill`

## 2. RegViewCache 增加 defs 缓存

- [ ] 2.1 `crates/modbussim-egui/src/app.rs` `RegViewCache` 结构加字段：`pub defs: Arc<std::collections::HashMap<u16, (String, String)>>`（addr → (name, comment)）
- [ ] 2.2 `refresh_reg_view`（async 路径）填充 defs：遍历 `dev.register_defs.iter().filter(|d| d.register_type == reg_type)`，把 `(d.address, (d.name.clone(), d.comment.clone()))` 插 HashMap
- [ ] 2.3 `RegViewCache::new()` 或初始化处给 defs 默认 Arc::new(HashMap::new())
- [ ] 2.4 `cargo build -p modbussim-egui` 通过
- [ ] 2.5 commit: `feat(slave): RegViewCache 缓存 register_defs 的 name/comment`

## 3. Bool 分支 TableBuilder 列重配置 + toggle 渲染

- [ ] 3.1 `Selection::RegisterGroup` 的 is_bool 分支前，按 `is_bool` 分叉 TableBuilder 列配置：
  - `is_bool=true`: 4 列 `exact(80) / exact(170) / exact(200) / remainder`
  - `is_bool=false`: 保持现有 5 列 `exact(80) / exact(110) / exact(100) / exact(140) / remainder`
- [ ] 3.2 header 行按 is_bool 分叉文案：bool 显示 `地址 / 值 / 名称 / 注释`（不再显示 `布尔` / `—` / `—` / `空`）
- [ ] 3.3 body 行 bool 分支里：
  - 第 1 列（地址）保持现有 SelectableLabel 点击支持 + row_clicks
  - 第 2 列：`let mut tmp = current; if toggle_switch(ui, flavor, &mut tmp).clicked() && tmp != current { writes.push((addr, if tmp { 1 } else { 0 })); pending.remove(&key); }`
  - 第 3 列：`let name = view.defs.get(&addr).map(|(n, _)| n.as_str()).unwrap_or(""); if !name.is_empty() { ui.monospace(name); }`
  - 第 4 列：同上但取 comment
  - 删除旧的自绘 ○/● + ON/OFF 代码块
- [ ] 3.4 `cargo build -p modbussim-egui` 通过
- [ ] 3.5 启动 Slave 手测：FC01 view 表头显示 `地址/值/名称/注释`；点 toggle 轨道翻转；hover 时滑块放大；"清空/关闭" 等次级按钮静止态肉眼可见
- [ ] 3.6 commit: `style(slave): FC01/FC02 表格改 4 列（地址/值/名称/注释）+ toggle 开关`

## 4. 回归 + push

- [ ] 4.1 `cargo test --workspace --exclude modbussim-app --exclude modbusmaster-app` 全绿
- [ ] 4.2 `cargo build --release -p modbussim-egui -p modbusmaster-egui` 通过
- [ ] 4.3 mbpoll 冒烟：`mbpoll -m tcp -a 1 -r 0 -c 5 127.0.0.1 -p 5502 -t 0 -- 1 0 1 0 1` 写 coil，UI 中 addr 0-4 五个 toggle 应交替 ON/OFF
- [ ] 4.4 `openspec-cn validate bool-view-and-button-polish`
- [ ] 4.5 `git push origin refactor/egui-skeleton` 观察 CI
