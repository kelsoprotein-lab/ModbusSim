# 中英文切换 (i18n) 设计

- 日期: 2026-05-01
- 范围: `shared-frontend` (新增 i18n 模块) + `master-frontend` + `frontend` + `crates/modbussim-app/src/commands.rs` 一处默认站名
- 状态: Draft (待用户审阅)
- 参考: 上一级目录 `IEC60870-5-104-Simulator/docs/superpowers/specs/2026-04-27-bilingual-i18n-design.md` (本设计沿用其方案,做 ModbusSim 适配)

## 背景与目标

ModbusSim 主从两个 Tauri 前端 (`master-frontend` / `frontend`) 共 11 个 `.vue` 文件硬编码中文 (~270 行),Tauri 后端 `crates/modbussim-app/src/commands.rs` 仅 1 处默认站名 `"从站 1"`。需要在运行时支持中英文双语切换。

egui 端 (`crates/modbussim-egui` & `modbusmaster-egui`) 已通过 `crates/modbussim-ui-shared/src/i18n.rs` 独立完成 i18n,**不在本次范围**。

目标:

1. 主从两端 Tauri 前端均支持中英文动态切换
2. 切换覆盖前端所有 UI 文案 + 后端默认站名展示
3. 跟随系统语言初始化,用户切换后持久化到 localStorage
4. 切换 UI 简单可见,一键即可
5. 不引入额外依赖,保持现有 Vue 3 + TS + Tauri 2 技术栈
6. 利用既有 `shared-frontend` workspace 共享一份字典,避免主从漂移

## 范围

### 包含

- `master-frontend/src/**/*.vue` 与 `frontend/src/**/*.vue` 中所有硬编码中文(按钮、菜单、对话框、表头、状态文字、错误提示、tooltip)
- `crates/modbussim-app/src/commands.rs:186-187` 默认站名 `"从站 1"` 改为后端发空字符串、前端 `t('station.defaultName', { id })` 拼接
- `shared-frontend` 新增 `i18n/` 子模块,导出 `useI18n` composable + 字典

### 不包含 (YAGNI)

- 复数形式
- 日期/数字本地化格式 (项目使用固定 hex/数字)
- 三种及以上语言 (仅中英两种)
- Tauri 后端 tracing 日志本地化
- README / CHANGELOG / 应用元数据本地化
- egui 端 i18n (已独立存在,不变更)
- `crates/modbussim-core` 的类型/分类显示字符串 (LogPanel 已是结构化数据,无中文文本)
- 后端 `LogEntry` 字段本地化 (FunctionCode + hex detail,与显示语言无关)

## 决策摘要

| 决策点 | 结果 |
| --- | --- |
| 本地化范围 | Tauri 前端 UI + 后端默认站名 |
| 默认语言 | 跟随系统语言 (`navigator.language` 以 `zh` 开头 → `zh-CN`,否则 `en-US`) |
| 持久化 | `localStorage['modbussim.locale']` |
| 切换 UI 位置 | 工具栏右侧,`toolbar-spacer` 之后、`toolbar-title` 之前 |
| 切换控件形态 | 紧凑 toggle 按钮 `[ 中 \| EN ]` |
| i18n 方案 | 自研轻量 composable (无新依赖) |
| 字典存放 | `shared-frontend/src/i18n/` (主从共用) |
| 后端默认站名处理 | 后端发空 String,前端按 locale 拼接展示 |

## 架构

### 目录结构

```
shared-frontend/
  src/
    i18n/
      index.ts          # useI18n() + 启动初始化
      detect.ts         # navigator.language → 'zh-CN' | 'en-US'
      types.ts          # Locale, MessageSchema
      locales/
        zh-CN.ts        # 字典 (as const, 单一事实源)
        en-US.ts        # 类型 = typeof zhCN, 缺键编译失败
    index.ts            # 在已有的导出上追加: export { useI18n, type Locale } from './i18n'
  tests/
    i18n.spec.ts        # Vitest 单元测试 (新增)
```

`master-frontend` / `frontend` 通过 `import { useI18n } from 'shared-frontend'` 使用,不持有自己的字典副本。

## Composable 设计

### 公开 API

```ts
import { useI18n } from 'shared-frontend'

const { t, locale, setLocale } = useI18n()

t('toolbar.connect')                       // → '连接' / 'Connect'
t('station.defaultName', { id: 1 })        // → '从站 1' / 'Slave 1'
locale.value                               // ref<'zh-CN' | 'en-US'>
setLocale('en-US')                         // 切换 + 写 localStorage
```

### 实现要点

- `locale` 为模块级 `ref<Locale>` (全局单例,所有组件共享)
- `t(key, params?)`:在 `MESSAGES[locale.value]` 中按点分路径查 key;未命中按 `当前 locale → en-US → 原 key 字符串` 三级回退;命中后对结果中 `{name}` 占位符做参数替换
- 占位符语法仅 `{name}`,无嵌套、无格式化函数 (覆盖项目所有需求)
- `t` 在模板中调用时自动响应 `locale` 变化 (依赖 Vue 3 响应式)
- 启动初始化顺序:
  1. `localStorage.getItem('modbussim.locale')` 若是合法 Locale 则使用
  2. 否则读 `navigator.language`,以 `zh` 开头 → `zh-CN`
  3. 否则默认 `en-US`
- `setLocale(next)` 写 `locale.value` 并同步 `localStorage`

## 字典组织

按组件 / 功能域分 namespace。键名 camelCase。两端共有的键 (`confirm`、`cancel`、`start`、`stop`、`save` 等) 放 `common.*`。

```ts
// shared-frontend/src/i18n/locales/zh-CN.ts (示例)
export default {
  common: {
    confirm: '确认', cancel: '取消', ok: '确定', save: '保存',
    delete: '删除', edit: '编辑', refresh: '刷新', close: '关闭',
    yes: '是', no: '否',
  },
  toolbar: {
    open: '打开', save: '保存', saveAs: '另存为',
    newConnection: '新建连接', addStation: '添加站',
    connect: '连接', disconnect: '断开',
    cancelReconnect: '取消重连',
    addScanGroup: '扫描组', startAll: '全部启动', stopAll: '全部停止',
    write: '写入', scan: '扫描',
    appTitleMaster: 'ModbusMaster', appTitleSlave: 'ModbusSim',
  },
  tree: { servers: '服务器', station: '站', noConnection: '暂无连接' },
  status: {
    connected: '已连接', disconnected: '已断开',
    reconnecting: '重连中', error: '错误',
    running: '运行中', stopped: '已停止',
  },
  station: { defaultName: '从站 {id}' },
  table: {
    address: '地址', value: '值', type: '类型', name: '名称',
    holdingRegister: '保持寄存器', inputRegister: '输入寄存器',
    coil: '线圈', discreteInput: '离散输入',
  },
  log: { tx: '发送', rx: '接收', clearLogs: '清空日志', exportCsv: '导出 CSV' },
  dialog: {
    transport: '传输类型', host: '主机', port: '端口', slaveId: '从站 ID',
    timeout: '超时 (ms)', baudRate: '波特率', dataBits: '数据位',
    stopBits: '停止位', parity: '校验', serialPort: '串口',
    enableTls: '启用 TLS',
    /* ... 完整列表在实施时按 grep 结果落实 */
  },
  errors: {
    invalidPort: '请输入有效的端口号 (1-65535)',
    invalidSlaveId: '请输入有效的从站 ID (1-247)',
    serialPortRequired: '请选择串口',
    invalidFloat: '包含无效的浮点数',
    /* ... 与 .vue 中现有提示一一对齐 */
  },
} as const
```

`en-US.ts` 提供等价的英文翻译,键集合与中文严格一致。

### 字典完整性保证

- TypeScript:`zh-CN.ts` 用 `as const` 导出
- `en-US.ts` 类型声明为 `typeof zhCN`,缺键时 `vue-tsc` 直接报错
- 新增中文键忘记加英文翻译会被构建时拦下

## 切换 UI

### 位置

两端 `Toolbar.vue` 中 `toolbar-spacer` 之后、`toolbar-title` 之前各放一份独立组件,但共用 `useI18n` 与字典。

### 形态

紧凑双栏 toggle:

```
[ 中 | EN ]
```

- 当前 locale 一侧高亮 (沿用现有 `#313244` / `#cdd6f4` 色板,与 `.toolbar-btn` 一致)
- 点击未高亮一侧立即切换;选中一侧再点无副作用
- 高度对齐 toolbar (~22px),水平 padding 与 `.toolbar-btn` 一致
- 鼠标悬停 tooltip:中 → "切换到中文" / EN → "Switch to English"

### 行为

- 点击立即切换 `locale.value` → 全 UI 通过 `t()` 响应式更新
- 同步写 `localStorage`
- 已显示的列表项 / LogPanel 条目立即重渲染为新语言

## 后端默认站名

### 现状

`crates/modbussim-app/src/commands.rs:186-187`:

```rust
Some("random") => SlaveDevice::with_random_registers(1, "从站 1", 20000),
_ => SlaveDevice::with_default_registers(1, "从站 1", 20000),
```

### 改造

后端创建时 name 留空字符串:

```rust
Some("random") => SlaveDevice::with_random_registers(1, "", 20000),
_ => SlaveDevice::with_default_registers(1, "", 20000),
```

### 前端展示

在 `frontend/src/components/ConnectionTree.vue` 等渲染 station 名称之处:

```ts
const displayName = (dev: { name: string; slave_id: number }) =>
  dev.name?.trim() || t('station.defaultName', { id: dev.slave_id })
```

→ name 非空照常展示用户填的名字;name 为空则按当前 locale 拼接 (`从站 {id}` / `Slave {id}`)。切语言时即时生效。

### 兼容性

- `SlaveDevice::with_*_registers` 签名不动 (`impl Into<String>`)
- 其他使用方 (egui 端、CLI、测试) 若传非空名字则展示用户值,行为不变

## 测试

### Vitest 单元测试 (`shared-frontend/tests/i18n.spec.ts`,新增)

- `detect.ts`:mock `navigator.language` 为 `zh-CN` / `zh-TW` / `en-US` / `ja-JP`,断言返回对应 Locale 或默认 `en-US`
- `useI18n`:
  - `t('toolbar.connect')` 在 zh-CN 返回中文、en-US 返回英文
  - `t('station.defaultName', { id: 7 })` 占位符替换正确
  - 未知 key 回退:zh 缺失时尝试 en,都缺时返回原 key 字符串
  - `setLocale('en-US')` 后 `localStorage.getItem('modbussim.locale')` 为 `'en-US'`
  - 启动初始化优先级:localStorage 命中 > 系统语言 > 默认

### 类型测试 (构建时)

- `vue-tsc -b` 必须在 `en-US.ts` 缺 key 时报错 (实施期间手动验证一次)

### 集成 / 手工 E2E

- 中文系统下首次启动 → 默认中文
- 英文系统下首次启动 → 默认英文
- 切到英文 → reload → 依然英文
- 主端 (master-frontend):遍历工具栏、新建连接对话框、写入对话框、扫描对话框、删除确认 — 所有可见文本切换无遗漏
- 从端 (frontend):遍历工具栏、批量添加、寄存器编辑、值面板、工具页 — 同上
- 默认站名 `从站 1` ↔ `Slave 1` 切换即时生效
- LogPanel 的 TX/RX 标签、清空/导出按钮跟随切换

## 风险与缓解

| 风险 | 缓解 |
| --- | --- |
| 漏掉 .vue 中的硬编码中文 | 实施时对每个 `.vue` 文件 `grep '[一-龥]'` 清零;CI 可加 `scripts/check-i18n.sh` 做回归保护 |
| en-US 字典与 zh-CN 漂移 | TypeScript `typeof zhCN` 类型约束,缺键编译失败 |
| 英文文本通常比中文长 1.5x,布局溢出 | 实施后逐页人工巡检,必要时加 `min-width` / 截断省略 |
| `shared-frontend` 改动后两端构建顺序 | `package.json` workspaces 已配置,`vite` 自动处理依赖 |
| egui 端独立 i18n 与本次改动产生混淆 | 在 README / 本 spec 注明:Tauri 前端 i18n 与 egui i18n 是两套独立体系,不共字典 |
| 已存储为某种语言的 station name 切换 locale 失效 | 默认站名走"后端空 + 前端拼接"路径;用户填写的名字按用户值原样展示 (设计上不强制翻译) |

## 实施顺序建议 (供 writing-plans 参考)

1. `shared-frontend/src/i18n/`:`detect.ts` + `types.ts` + `index.ts` + `locales/{zh-CN,en-US}.ts` 骨架
2. `shared-frontend` 在 `index.ts` 追加 `useI18n` 导出;补 Vitest 单元测试 (无 vitest 配置则一并加)
3. `master-frontend/src/components/Toolbar.vue` 接入 toggle 按钮 + 该文件文案迁移 (作为参考实现)
4. master-frontend 其余组件文案迁移:`ConnectionTree` → `DataTable` → `ValuePanel` → `LogPanel` → `ScanDialog` → 各 Modal
5. `frontend/src/components/Toolbar.vue` 接入 toggle + 该文件文案迁移
6. frontend 其余组件文案迁移:`ConnectionTree` → `RegisterTable` → `RegisterModal` → `BatchAddModal` → `ValuePanel` → `LogPanel` → `ToolsView`
7. `crates/modbussim-app/src/commands.rs` 默认站名清理 + 前端 `displayName` 拼接接入
8. `vue-tsc -b` 与单元测试全绿
9. 手工 E2E 巡检中英双向 + 持久化 + 系统语言

## 与 104 项目方案的差异

| 维度 | 104 | ModbusSim (本方案) |
| --- | --- | --- |
| 字典存放 | 各前端独立 (`master-frontend/src/i18n/`、`frontend/src/i18n/` 各一份) | 共享 `shared-frontend/src/i18n/`,两端引用同一份 |
| 后端面向用户中文 | `commands.rs` ~7 处 (单点/双点/步调节/设定值等命令日志) | `commands.rs` 仅 1 处 (默认站名) |
| LogPanel 后端事件本地化 | 需要 (改 Rust 侧发结构化事件) | **不需要** (LogEntry 字段已是结构化 hex/数字) |
| persistence key | `iec104.locale` | `modbussim.locale` |
| 应用标题 | `IEC 104 Slave` / `IEC 104 Master` | `ModbusSim` / `ModbusMaster` |
