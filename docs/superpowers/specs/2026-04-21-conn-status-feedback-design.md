# 连接状态反馈与按钮分工 · Spec

## Context

子站 SidePanel 重设计落地后（PR #2），用户反馈两个剩余问题：

1. **左下角 footer 的「启动 / 删除连接」按钮不明显** —— 当前用 `link_action`（无边框灰字），视觉重量极低
2. **启动/停止操作没有明显的前端感知** —— 连接状态仅由树节点 label 末尾的文本 tag `[运行中]/[已停止]` 表示，无颜色/图标/动画。用户切换状态后除文字 tag 变化外没有反馈

底层还有一个隐藏问题：操作入口在两处冗余 —— 树节点内（`render_tree` line 1704-1721）已用 `ui.small_button` 渲染"启动/停止/删除"三连按钮，footer（line 3418-3441）又渲染了一遍。两套并存且都不够明显。

目标：重新分工两处入口（高频/低频）、加强连接运行态的整体视觉感知（圆点 + 整行染色 + 脉动）、给 destructive 操作加二次确认避免误删，全部基于已落地的 token 系统（success / danger / text_muted / accent）。

---

## 现状关键代码定位

| 区域 | 文件 / 行 | 当前形态 |
|---|---|---|
| 树节点连接行渲染 | `crates/modbussim-egui/src/app.rs` 1665-1701 | 文本 label `"{} [{}]"` 只染主题色，tag 同色无区分 |
| 树节点内启动/停止/删除 三连 | `app.rs` 1704-1721 | `ui.small_button` 三按钮平铺缩进 18px |
| SidePanel footer 启动/停止/删除 | `app.rs` 3418-3441 | `link_action` 灰字，依赖 hover 才变色 |
| 底部状态栏 ● 与文案 | `app.rs` 3454+（`status_bar` BottomPanel） | 静态 ● success / 文案 `就绪` |
| 状态枚举 | `app.rs`（`ConnectionState::Running` / `Stopped`） | 已有 |
| Backend 启停接口 | `app.rs` 1857-1859 | `start_connection / stop_connection / remove_connection` 已就绪 |
| `TreeAction` | 已有 `StartConn` / `StopConn` / `RemoveConn` | 不需改 |

---

## 设计

### 1 · 信息架构（按钮分工）

```
┌──────────────────────────────┐
│ 连接                  [+ 新建] │
├──────────────────────────────┤
│ ● TCP 0.0.0.0:5502  运行中    │ ← 整行 success @ 8% 染色
│   [■ 停止]                    │ ← 树节点：单按钮 outline + warn 字
│   ▼ 从站 1                    │
│     FC01 线圈 (20001)         │
│ ○ TCP 192.168.1.10:502 已停止 │ ← 不染色
│   [▶ 启动]                    │ ← 树节点：单按钮 outline + success 字
│   ▶ 从站 1                    │
├──────────────────────────────┤
│ [× 删除连接 TCP 0.0.0.0:5502] │ ← footer：仅 destructive，针对当前选中
└──────────────────────────────┘
```

- **树节点内**（`render_tree` line 1704-1721）：删除"删除"按钮；剩下的状态相关单按钮 `ui.small_button` → `secondary_button_sm`（Outline Sm），按钮文字含 icon + 颜色（运行 → `■ 停止` warn 黄字 / 停止 → `▶ 启动` success 绿字）
- **footer**（line 3418-3441）：去掉启动/停止；只留删除连接，改 `danger_button_sm`（新增 helper）+ 文字含选中连接名 → `× 删除连接 TCP 0.0.0.0:5502`
- **删除二次确认**：按一次 → 按钮文字变 `× 再点一次确认` 维持 3 秒；3 秒内再点 → 真删；3 秒外或切换连接 → 自动恢复 label。**不**起 modal dialog

### 2 · 状态指示

#### 圆点（每个 connection row 最左侧）

- 位置：`row_resp.rect.left_center() + (8.0, 0.0)`，固定 14px 槽位
- 半径：3.5px
- 运行中：`theme::success(flavor)` 实心 + 1.5s 周期 alpha 脉动 (180..=255)
- 停止：`theme::text_muted(flavor)` 1px 描边空心圆

```rust
// in render_tree, before connection row painter.text
let dot_center = row_resp.rect.left_center() + egui::vec2(8.0, 0.0);
match snap.state {
    ConnectionState::Running => {
        let phase = (ui.input(|i| i.time)
            * (2.0 * std::f64::consts::PI / 1.5)).sin() * 0.5 + 0.5;
        let alpha = (180.0 + 75.0 * phase) as u8;
        let mut c = theme::success(flavor);
        c = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha);
        ui.painter().circle_filled(dot_center, 3.5, c);
    }
    ConnectionState::Stopped => {
        ui.painter().circle_stroke(
            dot_center, 3.5,
            egui::Stroke::new(1.0, theme::text_muted(flavor)),
        );
    }
}
```

#### 整行染色（仅运行中）

- 范围：connection row 完整宽度（不含其下 small_button 行）
- 颜色：`Color32::from_rgba_unmultiplied(success.r, success.g, success.b, 0x14)` (8% alpha)
- 优先级：`if conn_is_selected { paint_active_row } else if running { paint_running_row } else if hover { paint_hover }` —— 三者互斥

```rust
let paint_running_row = |ui: &egui::Ui, rect: egui::Rect| {
    let s = theme::success(flavor);
    ui.painter().rect_filled(
        rect, 0.0,
        Color32::from_rgba_unmultiplied(s.r(), s.g(), s.b(), 0x14),
    );
};
```

#### 状态 tag 文本（line 1668-1672）

`[运行中]/[已停止]` → `运行中`/`已停止`（去括号），并染色：
- 运行 → `theme::success(flavor)`
- 停止 → `theme::text_muted(flavor)`

label 与 tag 分两次 `painter.text` 绘制（label 为主，tag 在右侧 6px 间距处）。

#### 底部状态栏（line 3454+）

- `any_running = self.conn_snapshot.iter().any(|s| s.state == ConnectionState::Running)`
- 至少一个运行 → `● 运行中 · N 连接 · M 从站`，● 与树节点同函数同相位脉动
- 全部停止 → `○ 已停止 · N 连接 · M 从站`，muted 静态
- 0 连接 → `○ 未连接`，muted 静态

### 3 · 瞬时反馈

不引入 toast。反馈完全靠"行染色 + 圆点脉动 + tag 染色 + 按钮文字变换"四通道同步：

- 用户点击"启动"瞬间 → state 立即 flip 到 `Running` → 下一帧整行立即变绿底 + 圆点开始脉动 + tag 变绿 + 按钮变 `■ 停止` 黄字
- backend 启动若失败 → 走现有 `clear_error` 路径，状态栏显示错误，state 回滚 `Stopped`，整行染色随之消失（**不动现有 error 处理**）

### 4 · 删除二次确认

新增 `SlaveApp` 字段：

```rust
pub pending_delete: Option<(String, std::time::Instant)>,
```

footer 按钮逻辑：

```rust
let now = std::time::Instant::now();
let confirming = self.pending_delete
    .as_ref()
    .filter(|(id, t)| id == &conn_id && now.duration_since(*t).as_secs_f32() < 3.0)
    .is_some();
let label: String = if confirming {
    "× 再点一次确认".to_string()
} else {
    format!("× 删除连接 {}", snap.label)
};
if uikit::danger_button_sm(ui, self.flavor, label).clicked() {
    if confirming {
        tree_action = Some(TreeAction::RemoveConn(conn_id.clone()));
        self.pending_delete = None;
    } else {
        self.pending_delete = Some((conn_id.clone(), now));
        ctx.request_repaint_after(std::time::Duration::from_millis(3100));
    }
}
```

`confirming` 检查同时绑定 `conn_id` 与 3 秒窗口，自动覆盖以下边界：
- 3 秒过 → `now.duration_since(t) >= 3.0` 为 false → 退出 confirming（旧字段值保留无害，下次点击会用新 instant 覆盖）
- 切换到别的连接 → footer 渲染时 `id != &conn_id` → 退出 confirming
- 选中态消失（无 active_conn）→ footer 整段不渲染，自然不进入分支

保留旧 `pending_delete` 字段值不会触发误删，因为只有"在同一连接 + 3 秒内 + 主动再点"三条件并发才会真删。无需额外清理代码。

### 5 · 性能

- 状态栏渲染时统一 `if any_running { ctx.request_repaint_after(50ms) }` 一处即可，驱动树+状态栏所有脉动
- 50ms ≈ 20fps 足够丝滑、CPU 几乎为零
- 全部连接停止时 → 不请求 repaint，UI 静止零负担

---

## 主要修改文件

- `crates/modbussim-ui-shared/src/ui.rs` — 新增 `danger_button_sm`（shadcn Destructive + Sm，与 `secondary_button_sm` 平行）
- `crates/modbussim-egui/src/app.rs`：
  - `SlaveApp` struct + Default：加 `pending_delete` 字段
  - `render_tree` line 1665-1721：圆点、整行染色、tag 染色、small_button → secondary_button_sm + icon、删按钮
  - SidePanel footer line 3418-3441：去启停、删除改 `danger_button_sm` + 二次确认
  - status_bar line 3454+：脉动 ● 与文案

---

## 验证

1. `cargo run -p modbussim-egui --release`
2. 新建一个 TCP 连接（端口 5502），观察：
   - 默认 `Stopped` → 行无染色、左侧 ○ 灰描边、tag 灰字「已停止」、节点内按钮 `▶ 启动` 绿字
3. 点击「启动」：
   - 整行立即 success 8% 绿底
   - ● 绿圆点出现并以 1.5s 周期脉动 alpha
   - tag 立即变 success 绿色「运行中」
   - 按钮文字立即变 `■ 停止` warn 黄字
   - 底部状态栏 `● 运行中 · 1 连接 · 1 从站` 同步脉动
4. 点击「停止」反向回退所有视觉
5. 点击 footer `× 删除连接 TCP 0.0.0.0:5502`：
   - 按钮文字变 `× 再点一次确认`
   - 3 秒内再点 → 真删
   - 3 秒外不点 → 自动恢复
6. 同时开 2 个连接，一开一停 → 树内一行染绿一行不染，状态栏脉动
7. 浅色模式切换：success 浅色（#15803d）@ 8% 在白底是浅绿，可辨；圆点描边正常
8. `cargo check --workspace` / `cargo clippy -p modbussim-egui -p modbussim-ui-shared --no-deps -- -D warnings` 干净
9. 浏览 `cargo run` 时的 CPU 占用：所有连接停止时应近 0%；运行中时 ≤ 5% 单核（脉动重绘）
