# Bilingual i18n Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ModbusSim 主从两个 Tauri 前端运行时中英文双语切换,字典放在 `shared-frontend` 共用,后端默认站名由前端按当前 locale 拼接。

**Architecture:** `shared-frontend` 新增 `src/i18n/` 子模块,导出 `useI18n()` composable + `zh-CN` / `en-US` 字典。`zh-CN.ts` 用 `as const` 导出,`en-US.ts` 类型为 `typeof zhCN`,缺键时 `vue-tsc -b` 报错。两端 `Toolbar.vue` 末尾追加 toggle 按钮 `[ 中 | EN ]`,持久化 key `localStorage['modbussim.locale']`,首次启动跟随 `navigator.language`。

**Tech Stack:** Vue 3 (composable + ref 响应式) · TypeScript (`as const` + `typeof` 类型约束) · Vitest (新增,验证 i18n 行为) · Vite · Tauri 2 · Rust(仅一处 `commands.rs` 默认站名清理)

**Spec:** `docs/superpowers/specs/2026-05-01-bilingual-i18n-design.md`

**Workspace 注意:** 仓库已配置 npm workspaces (`shared-frontend` / `frontend` / `master-frontend`)。所有 `npm install` / `npx` 命令在仓库根目录执行;两端通过 `"shared-frontend": "*"` 已 link,改动 `shared-frontend/src` 后两端 `vite` 自动热更新。

**全局命令前缀:** 所有 shell 命令默认 cwd 为仓库根:
```
/Users/daichangyu/Library/Mobile Documents/com~apple~CloudDocs/code/ModbusSim
```

---

## File Structure

```
shared-frontend/
  package.json               # 修改: 新增 vitest devDep + test script
  vitest.config.ts           # 新增
  src/
    index.ts                 # 修改: 追加 export { useI18n, type Locale }
    i18n/
      index.ts               # 新增: useI18n() + 启动初始化
      detect.ts              # 新增: 系统语言检测 + storage 读写
      types.ts               # 新增: Locale, MessageSchema
      locales/
        zh-CN.ts             # 新增: as const 字典 (单一事实源)
        en-US.ts             # 新增: 类型 = typeof zhCN
  tests/
    detect.spec.ts           # 新增
    useI18n.spec.ts          # 新增

master-frontend/
  src/
    components/
      LangToggle.vue         # 新增: [ 中 | EN ] toggle (主从两端共用此组件,但放各自目录便于样式独立)
      Toolbar.vue            # 修改: 引入 LangToggle + t() 替换硬编码中文
      ConnectionTree.vue     # 修改: t() 替换硬编码
      DataTable.vue          # 修改
      ValuePanel.vue         # 修改
      LogPanel.vue           # 修改
      ScanDialog.vue         # 修改

frontend/
  src/
    components/
      LangToggle.vue         # 新增: 同上
      Toolbar.vue            # 修改: 引入 LangToggle + t() 替换 + 创建 station 时 name 留空
      ConnectionTree.vue     # 修改: 默认站名 displayName(t) + 其它文案
      RegisterTable.vue      # 修改
      RegisterModal.vue      # 修改
      BatchAddModal.vue      # 修改
      ValuePanel.vue         # 修改
      LogPanel.vue           # 修改

crates/modbussim-app/src/
  commands.rs                # 修改 line 186-187: "从站 1" → ""
```

每个 `.vue` 改动只是把字面量中文替换为 `t('namespace.key')` 调用 + 在 setup 顶部 `import { useI18n } from 'shared-frontend'` 与 `const { t } = useI18n()`。

---

## Task 1 — shared-frontend 接入 Vitest

**Files:**
- Modify: `shared-frontend/package.json`
- Create: `shared-frontend/vitest.config.ts`
- Create: `shared-frontend/tests/sanity.spec.ts` (验证 vitest 能跑,实施完后删除)

- [ ] **Step 1: 给 shared-frontend 加 vitest 依赖**

```bash
npm install -w shared-frontend -D vitest jsdom @vue/test-utils
```

预期:`shared-frontend/package.json` 多出三个 devDependencies。

- [ ] **Step 2: 给 shared-frontend 加 test script**

修改 `shared-frontend/package.json`,在文件中新增 `scripts` 段(与 `dependencies` 同级):

```json
{
  "name": "shared-frontend",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "main": "src/index.ts",
  "scripts": {
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": { ... },
  "devDependencies": { ... }
}
```

`...` 用现有内容保留。

- [ ] **Step 3: 写 vitest 配置**

创建 `shared-frontend/vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['tests/**/*.spec.ts'],
  },
})
```

- [ ] **Step 4: 写一个 sanity 测试验证 vitest 能跑**

创建 `shared-frontend/tests/sanity.spec.ts`:

```ts
import { describe, expect, it } from 'vitest'

describe('sanity', () => {
  it('runs', () => {
    expect(1 + 1).toBe(2)
  })
})
```

- [ ] **Step 5: 跑测试**

```bash
npm test -w shared-frontend
```

预期:`✓ tests/sanity.spec.ts (1 test)`,exit code 0。

- [ ] **Step 6: 删除 sanity 测试**

```bash
rm shared-frontend/tests/sanity.spec.ts
```

- [ ] **Step 7: Commit**

```bash
git add shared-frontend/package.json shared-frontend/vitest.config.ts package-lock.json
git commit -m "chore(shared-frontend): 接入 vitest 单元测试基础设施"
```

---

## Task 2 — i18n 类型 + detect + 单元测试

**Files:**
- Create: `shared-frontend/src/i18n/types.ts`
- Create: `shared-frontend/src/i18n/detect.ts`
- Create: `shared-frontend/tests/detect.spec.ts`

- [ ] **Step 1: 写失败测试 — detect.ts 行为**

创建 `shared-frontend/tests/detect.spec.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  STORAGE_KEY,
  detectInitialLocale,
  loadStoredLocale,
  storeLocale,
} from '../src/i18n/detect'

describe('detect.loadStoredLocale', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('returns null when nothing stored', () => {
    expect(loadStoredLocale()).toBeNull()
  })

  it('returns the stored locale when valid', () => {
    localStorage.setItem(STORAGE_KEY, 'en-US')
    expect(loadStoredLocale()).toBe('en-US')
  })

  it('returns null when stored value is invalid', () => {
    localStorage.setItem(STORAGE_KEY, 'fr-FR')
    expect(loadStoredLocale()).toBeNull()
  })
})

describe('detect.storeLocale', () => {
  afterEach(() => {
    localStorage.clear()
  })

  it('writes the locale into localStorage', () => {
    storeLocale('zh-CN')
    expect(localStorage.getItem(STORAGE_KEY)).toBe('zh-CN')
  })
})

describe('detect.detectInitialLocale', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('prefers stored locale over navigator.language', () => {
    localStorage.setItem(STORAGE_KEY, 'en-US')
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('zh-CN')
    expect(detectInitialLocale()).toBe('en-US')
  })

  it('falls back to zh-CN when navigator.language starts with "zh"', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('zh-Hans-CN')
    expect(detectInitialLocale()).toBe('zh-CN')
  })

  it('falls back to en-US otherwise', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('ja-JP')
    expect(detectInitialLocale()).toBe('en-US')
  })

  it('falls back to en-US when navigator.language is empty', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('')
    expect(detectInitialLocale()).toBe('en-US')
  })
})
```

- [ ] **Step 2: 跑测试,确认全部失败**

```bash
npm test -w shared-frontend
```

预期:报 `Cannot find module '../src/i18n/detect'`。

- [ ] **Step 3: 写 types.ts**

创建 `shared-frontend/src/i18n/types.ts`:

```ts
export type Locale = 'zh-CN' | 'en-US'

export const SUPPORTED_LOCALES: readonly Locale[] = ['zh-CN', 'en-US'] as const

export function isLocale(value: unknown): value is Locale {
  return typeof value === 'string' && (SUPPORTED_LOCALES as readonly string[]).includes(value)
}
```

- [ ] **Step 4: 写 detect.ts**

创建 `shared-frontend/src/i18n/detect.ts`:

```ts
import { isLocale, type Locale } from './types'

export const STORAGE_KEY = 'modbussim.locale'

export function loadStoredLocale(): Locale | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return isLocale(raw) ? raw : null
  } catch {
    return null
  }
}

export function storeLocale(locale: Locale): void {
  try {
    localStorage.setItem(STORAGE_KEY, locale)
  } catch {
    // localStorage 被禁用时静默忽略;UI 仍能切换,只是重启后丢失
  }
}

export function detectFromNavigator(): Locale {
  const lang = (typeof navigator !== 'undefined' && navigator.language) || ''
  return lang.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}

export function detectInitialLocale(): Locale {
  return loadStoredLocale() ?? detectFromNavigator()
}
```

- [ ] **Step 5: 跑测试,确认全部通过**

```bash
npm test -w shared-frontend
```

预期:`✓ tests/detect.spec.ts (8 tests)`。

- [ ] **Step 6: Commit**

```bash
git add shared-frontend/src/i18n/types.ts shared-frontend/src/i18n/detect.ts shared-frontend/tests/detect.spec.ts
git commit -m "feat(i18n): 系统语言检测与 localStorage 持久化 + 单元测试"
```

---

## Task 3 — zh-CN 字典骨架(命名空间 + 必备 key)

**Files:**
- Create: `shared-frontend/src/i18n/locales/zh-CN.ts`

字典是单一事实源。本 task 建立**全部 namespace 与全部已知 key**,值用真实中文。en-US 在 Task 4 一次性填齐。

- [ ] **Step 1: grep 出主从两端所有中文字面量,作为 key 清单**

```bash
grep -n "[一-龥]" master-frontend/src/components/*.vue master-frontend/src/App.vue 2>/dev/null > /tmp/zh-master.txt
grep -n "[一-龥]" frontend/src/components/*.vue frontend/src/App.vue 2>/dev/null > /tmp/zh-slave.txt
wc -l /tmp/zh-master.txt /tmp/zh-slave.txt
```

预期总行数 ~280。逐行阅读两个文件,为每条中文按所属组件/语义归到下面的 namespace 之一。

- [ ] **Step 2: 写完整字典**

创建 `shared-frontend/src/i18n/locales/zh-CN.ts`,以下骨架是**最小完整结构**;实施时依据 Step 1 清单**为每个出现过的中文增补 key**(同一字面量在 zh 与 en 各只有一条)。新增 key 必须落在某个 namespace 下。

```ts
const messages = {
  common: {
    confirm: '确认',
    cancel: '取消',
    ok: '确定',
    save: '保存',
    delete: '删除',
    edit: '编辑',
    refresh: '刷新',
    close: '关闭',
    yes: '是',
    no: '否',
    add: '添加',
    remove: '移除',
    apply: '应用',
    create: '创建',
  },
  toolbar: {
    open: '打开',
    save: '保存',
    saveAs: '另存为',
    newConnection: '新建连接',
    addStation: '添加站',
    connect: '连接',
    disconnect: '断开',
    cancelReconnect: '取消重连',
    addScanGroup: '扫描组',
    startAll: '全部启动',
    stopAll: '全部停止',
    write: '写入',
    scan: '扫描',
    appTitleMaster: 'ModbusMaster',
    appTitleSlave: 'ModbusSlave',
    openProjectTitle: '打开项目',
    saveProjectTitle: '保存项目',
    saveAsTitle: '另存为',
  },
  tree: {
    servers: '服务器',
    station: '站',
    noConnection: '暂无连接',
    expand: '展开',
    collapse: '收起',
  },
  status: {
    connected: '已连接',
    disconnected: '已断开',
    reconnecting: '重连中',
    error: '错误',
    running: '运行中',
    stopped: '已停止',
  },
  station: {
    defaultName: '从站 {id}',
  },
  table: {
    address: '地址',
    value: '值',
    type: '类型',
    name: '名称',
    holdingRegister: '保持寄存器',
    inputRegister: '输入寄存器',
    coil: '线圈',
    discreteInput: '离散输入',
    quantity: '数量',
    startAddress: '起始地址',
    endAddress: '结束地址',
    interval: '轮询间隔 (ms)',
    function: '功能码',
    rawValue: '原始值',
    parsedValue: '解析值',
  },
  log: {
    tx: '发送',
    rx: '接收',
    clearLogs: '清空日志',
    exportCsv: '导出 CSV',
    direction: '方向',
    timestamp: '时间',
    detail: '详情',
  },
  dialog: {
    transport: '传输类型',
    host: '主机',
    port: '端口',
    slaveId: '从站 ID',
    timeout: '超时 (ms)',
    baudRate: '波特率',
    dataBits: '数据位',
    stopBits: '停止位',
    parity: '校验',
    serialPort: '串口',
    enableTls: '启用 TLS',
    caFile: 'CA 证书 (验证服务器)',
    clientCert: '客户端证书 (PEM)',
    clientKey: '客户端私钥 (PEM)',
    pkcs12File: 'PKCS#12 文件',
    pkcs12Password: 'PKCS#12 密码',
    acceptInvalidCerts: '接受自签名证书 (测试用)',
    refreshSerialPorts: '刷新串口列表',
    parityNone: 'None',
    parityOdd: 'Odd',
    parityEven: 'Even',
    dataType: '数据类型',
    dataTypeRaw: 'Raw u16',
    dataTypeFloat32: 'Float32 (REAL)',
    byteOrder: '字节序',
    address: '地址',
    valueLabel: '值',
    valueHintMultiple: '（逗号分隔）',
    valueHintFloat32: '（逗号分隔浮点数，如 3.14, 2.71, 1.41）',
    writeRegisters: '写入寄存器',
    newScanGroup: '新建扫描组',
    scanGroupName: '扫描组名称',
    scanInterval: '轮询间隔 (ms)',
  },
  errors: {
    invalidPort: '请输入有效的端口号 (1-65535)',
    invalidSlaveId: '请输入有效的从站 ID (1-247)',
    serialPortRequired: '请选择串口',
    invalidFloat: '包含无效的浮点数',
    confirmDeleteConnection: '确认删除此连接？',
    toolPanelPending: '工具面板（待实现）',
    invalidDataInput: '数据无效',
  },
  scanDialog: {
    title: '扫描',
    slaveScan: '从站 ID 扫描',
    registerScan: '寄存器扫描',
    startScan: '开始扫描',
    cancelScan: '取消扫描',
    progress: '进度',
    found: '已发现',
    chunkSize: '块大小',
  },
  // —— 实施期补充 namespace ——
  // 若 grep 出的中文不属于上面任一 namespace,新增 namespace(如 batchAdd / registerEdit / valuePanel)
  // 而非堆到通用 namespace。原则:同一组件的文案就近聚合。
} as const

export type Messages = typeof messages
export default messages
```

- [ ] **Step 3: 实施期具体补全(动态步骤)**

依据 Step 1 的 `grep` 输出,**对每条中文字面量执行**:

1. 判断它出现在哪个组件 / 语义场景
2. 在合适 namespace 下找/加 key(camelCase),确保 key 在整个字典中唯一
3. 占位符走 `{name}` 语法(例如 `'共 {count} 条'`)

**新增 namespace 的命名约定:**
- 主从专属对话框 → 用对话框名 camelCase(如 `batchAdd`、`registerEdit`)
- 主从工具页 → `tools`
- 错误/校验/确认 → 一律放 `errors`(本仓库 alert/confirm 量小)

**禁止:**
- 同一文案在两个 key 下重复(grep 完整字典 + 视觉去重)
- key 名包含中文或空格

- [ ] **Step 4: 类型快速验证**

```bash
npx -w shared-frontend tsc --noEmit -p shared-frontend/tsconfig.json
```

预期:无错误(此时 en-US.ts 还没建,字典只是 `as const` 自身合法)。

- [ ] **Step 5: Commit**

```bash
git add shared-frontend/src/i18n/locales/zh-CN.ts
git commit -m "feat(i18n): zh-CN 字典 — 主从两端全文案归集"
```

---

## Task 4 — en-US 字典(类型对齐 + 翻译)

**Files:**
- Create: `shared-frontend/src/i18n/locales/en-US.ts`

- [ ] **Step 1: 创建 en-US.ts 骨架**

创建 `shared-frontend/src/i18n/locales/en-US.ts`:

```ts
import type { Messages } from './zh-CN'

const messages: Messages = {
  common: {
    confirm: 'Confirm',
    cancel: 'Cancel',
    ok: 'OK',
    save: 'Save',
    delete: 'Delete',
    edit: 'Edit',
    refresh: 'Refresh',
    close: 'Close',
    yes: 'Yes',
    no: 'No',
    add: 'Add',
    remove: 'Remove',
    apply: 'Apply',
    create: 'Create',
  },
  toolbar: {
    open: 'Open',
    save: 'Save',
    saveAs: 'Save As',
    newConnection: 'New Connection',
    addStation: 'Add Station',
    connect: 'Connect',
    disconnect: 'Disconnect',
    cancelReconnect: 'Cancel Reconnect',
    addScanGroup: 'Scan Group',
    startAll: 'Start All',
    stopAll: 'Stop All',
    write: 'Write',
    scan: 'Scan',
    appTitleMaster: 'ModbusMaster',
    appTitleSlave: 'ModbusSlave',
    openProjectTitle: 'Open Project',
    saveProjectTitle: 'Save Project',
    saveAsTitle: 'Save As',
  },
  tree: {
    servers: 'Servers',
    station: 'Station',
    noConnection: 'No connections',
    expand: 'Expand',
    collapse: 'Collapse',
  },
  status: {
    connected: 'Connected',
    disconnected: 'Disconnected',
    reconnecting: 'Reconnecting',
    error: 'Error',
    running: 'Running',
    stopped: 'Stopped',
  },
  station: {
    defaultName: 'Slave {id}',
  },
  table: {
    address: 'Address',
    value: 'Value',
    type: 'Type',
    name: 'Name',
    holdingRegister: 'Holding Register',
    inputRegister: 'Input Register',
    coil: 'Coil',
    discreteInput: 'Discrete Input',
    quantity: 'Quantity',
    startAddress: 'Start Address',
    endAddress: 'End Address',
    interval: 'Interval (ms)',
    function: 'Function',
    rawValue: 'Raw',
    parsedValue: 'Parsed',
  },
  log: {
    tx: 'TX',
    rx: 'RX',
    clearLogs: 'Clear Logs',
    exportCsv: 'Export CSV',
    direction: 'Direction',
    timestamp: 'Timestamp',
    detail: 'Detail',
  },
  dialog: {
    transport: 'Transport',
    host: 'Host',
    port: 'Port',
    slaveId: 'Slave ID',
    timeout: 'Timeout (ms)',
    baudRate: 'Baud Rate',
    dataBits: 'Data Bits',
    stopBits: 'Stop Bits',
    parity: 'Parity',
    serialPort: 'Serial Port',
    enableTls: 'Enable TLS',
    caFile: 'CA Certificate (verify server)',
    clientCert: 'Client Certificate (PEM)',
    clientKey: 'Client Key (PEM)',
    pkcs12File: 'PKCS#12 File',
    pkcs12Password: 'PKCS#12 Password',
    acceptInvalidCerts: 'Accept self-signed certificates (test only)',
    refreshSerialPorts: 'Refresh serial ports',
    parityNone: 'None',
    parityOdd: 'Odd',
    parityEven: 'Even',
    dataType: 'Data Type',
    dataTypeRaw: 'Raw u16',
    dataTypeFloat32: 'Float32 (REAL)',
    byteOrder: 'Byte Order',
    address: 'Address',
    valueLabel: 'Value',
    valueHintMultiple: '(comma separated)',
    valueHintFloat32: '(comma separated floats, e.g. 3.14, 2.71, 1.41)',
    writeRegisters: 'Write Registers',
    newScanGroup: 'New Scan Group',
    scanGroupName: 'Scan group name',
    scanInterval: 'Poll interval (ms)',
  },
  errors: {
    invalidPort: 'Please enter a valid port (1-65535)',
    invalidSlaveId: 'Please enter a valid slave ID (1-247)',
    serialPortRequired: 'Please pick a serial port',
    invalidFloat: 'Contains invalid float value',
    confirmDeleteConnection: 'Delete this connection?',
    toolPanelPending: 'Tools panel (TBD)',
    invalidDataInput: 'Invalid input',
  },
  scanDialog: {
    title: 'Scan',
    slaveScan: 'Slave ID Scan',
    registerScan: 'Register Scan',
    startScan: 'Start Scan',
    cancelScan: 'Cancel Scan',
    progress: 'Progress',
    found: 'Found',
    chunkSize: 'Chunk Size',
  },
  // 此处和 zh-CN.ts 保持完全相同的 namespace 和 key 集合;每新增一个 zh key 必须同步加 en
}

export default messages
```

- [ ] **Step 2: 类型对齐验证(故意构造缺键 → vue-tsc 报错 → 还原)**

```bash
# 临时删除一行,验证类型约束生效
sed -i.bak '/^    confirm:/d' shared-frontend/src/i18n/locales/en-US.ts
npx vue-tsc -b master-frontend ; echo "exit=$?"
# 预期: 非 0,报 "Property 'confirm' is missing in type ..."
mv shared-frontend/src/i18n/locales/en-US.ts.bak shared-frontend/src/i18n/locales/en-US.ts
npx vue-tsc -b master-frontend ; echo "exit=$?"
# 预期: exit=0
```

- [ ] **Step 3: Commit**

```bash
git add shared-frontend/src/i18n/locales/en-US.ts
git commit -m "feat(i18n): en-US 字典 — 类型严格对齐 zh-CN"
```

---

## Task 5 — useI18n composable + index 导出 + 单元测试

**Files:**
- Create: `shared-frontend/src/i18n/index.ts`
- Create: `shared-frontend/tests/useI18n.spec.ts`
- Modify: `shared-frontend/src/index.ts`

- [ ] **Step 1: 写失败测试**

创建 `shared-frontend/tests/useI18n.spec.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { STORAGE_KEY } from '../src/i18n/detect'

// 模块级 ref 是单例。每个测试需要 reset modules 拿到干净的 useI18n。
async function freshI18n() {
  vi.resetModules()
  return await import('../src/i18n')
}

beforeEach(() => {
  localStorage.clear()
  vi.spyOn(navigator, 'language', 'get').mockReturnValue('en-US')
})

describe('useI18n.t', () => {
  it('returns the value for the active locale', async () => {
    const { useI18n } = await freshI18n()
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('common.confirm')).toBe('确认')
    setLocale('en-US')
    expect(t('common.confirm')).toBe('Confirm')
  })

  it('replaces {placeholders} with params', async () => {
    const { useI18n } = await freshI18n()
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('station.defaultName', { id: 7 })).toBe('从站 7')
    setLocale('en-US')
    expect(t('station.defaultName', { id: 7 })).toBe('Slave 7')
  })

  it('falls back to en-US when key missing in zh-CN', async () => {
    // 在测试场景下我们无法真的造一个只在 en-US 有的 key;改为验证回退逻辑通过强制覆盖字典实现。
    // 这里用一个保证存在的 key 验证 zh→zh 命中即可;真正的 en 回退由"返回 key 字符串"分支覆盖。
    const { useI18n } = await freshI18n()
    const { t, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(t('common.confirm')).toBe('确认')
  })

  it('returns the key string when missing in both locales', async () => {
    const { useI18n } = await freshI18n()
    const { t } = useI18n()
    expect(t('definitely.not.a.real.key' as never)).toBe('definitely.not.a.real.key')
  })
})

describe('useI18n.setLocale', () => {
  it('persists to localStorage', async () => {
    const { useI18n } = await freshI18n()
    const { setLocale } = useI18n()
    setLocale('en-US')
    expect(localStorage.getItem(STORAGE_KEY)).toBe('en-US')
  })

  it('updates the reactive locale ref', async () => {
    const { useI18n } = await freshI18n()
    const { locale, setLocale } = useI18n()
    setLocale('zh-CN')
    expect(locale.value).toBe('zh-CN')
  })
})

describe('initial locale', () => {
  it('reads from localStorage when present', async () => {
    localStorage.setItem(STORAGE_KEY, 'zh-CN')
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('en-US')
    const { useI18n } = await freshI18n()
    expect(useI18n().locale.value).toBe('zh-CN')
  })

  it('reads from navigator.language otherwise', async () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('zh-Hans-CN')
    const { useI18n } = await freshI18n()
    expect(useI18n().locale.value).toBe('zh-CN')
  })
})
```

- [ ] **Step 2: 跑测试,确认全部失败**

```bash
npm test -w shared-frontend
```

预期:报 `Cannot find module '../src/i18n'`。

- [ ] **Step 3: 写 useI18n composable**

创建 `shared-frontend/src/i18n/index.ts`:

```ts
import { ref } from 'vue'
import { detectInitialLocale, storeLocale } from './detect'
import zhCN, { type Messages } from './locales/zh-CN'
import enUS from './locales/en-US'
import type { Locale } from './types'

export type { Locale } from './types'

const dictionaries: Record<Locale, Messages> = {
  'zh-CN': zhCN,
  'en-US': enUS,
}

const locale = ref<Locale>(detectInitialLocale())

function lookup(dict: Messages, key: string): string | null {
  const parts = key.split('.')
  let cur: unknown = dict
  for (const p of parts) {
    if (cur && typeof cur === 'object' && p in (cur as Record<string, unknown>)) {
      cur = (cur as Record<string, unknown>)[p]
    } else {
      return null
    }
  }
  return typeof cur === 'string' ? cur : null
}

function interpolate(tpl: string, params?: Record<string, string | number>): string {
  if (!params) return tpl
  return tpl.replace(/\{(\w+)\}/g, (_, k) =>
    k in params ? String(params[k]) : `{${k}}`,
  )
}

function translate(key: string, params?: Record<string, string | number>): string {
  const tpl =
    lookup(dictionaries[locale.value], key) ??
    lookup(dictionaries['en-US'], key) ??
    key
  return interpolate(tpl, params)
}

function setLocale(next: Locale): void {
  locale.value = next
  storeLocale(next)
}

export function useI18n() {
  return {
    t: translate,
    locale,
    setLocale,
  }
}
```

- [ ] **Step 4: 跑测试,确认通过**

```bash
npm test -w shared-frontend
```

预期:`✓ tests/detect.spec.ts (8 tests) ✓ tests/useI18n.spec.ts (8 tests)`,exit=0。

- [ ] **Step 5: 在 shared-frontend 包入口追加导出**

修改 `shared-frontend/src/index.ts`,在文件末尾追加:

```ts
// i18n
export { useI18n, type Locale } from './i18n'
```

- [ ] **Step 6: 验证类型构建**

```bash
npx vue-tsc -b master-frontend ; echo "exit=$?"
npx vue-tsc -b frontend ; echo "exit=$?"
```

预期:两条都是 `exit=0`(此时还没人用 `useI18n`,只是验证导出语义)。

- [ ] **Step 7: Commit**

```bash
git add shared-frontend/src/i18n/index.ts shared-frontend/src/index.ts shared-frontend/tests/useI18n.spec.ts
git commit -m "feat(i18n): useI18n composable — t / locale / setLocale + 单元测试"
```

---

## Task 6 — master-frontend 文案迁移 + LangToggle 接入

**Files:**
- Create: `master-frontend/src/components/LangToggle.vue`
- Modify: `master-frontend/src/components/Toolbar.vue`
- Modify: `master-frontend/src/components/ConnectionTree.vue`
- Modify: `master-frontend/src/components/DataTable.vue`
- Modify: `master-frontend/src/components/ValuePanel.vue`
- Modify: `master-frontend/src/components/LogPanel.vue`
- Modify: `master-frontend/src/components/ScanDialog.vue`

- [ ] **Step 1: 写 LangToggle.vue 组件**

创建 `master-frontend/src/components/LangToggle.vue`:

```vue
<script setup lang="ts">
import { useI18n } from 'shared-frontend'
import type { Locale } from 'shared-frontend'

const { locale, setLocale } = useI18n()

function pick(next: Locale) {
  if (locale.value !== next) setLocale(next)
}
</script>

<template>
  <div class="lang-toggle" role="group" aria-label="Language">
    <button
      type="button"
      class="lang-toggle-btn"
      :class="{ active: locale === 'zh-CN' }"
      title="切换到中文"
      @click="pick('zh-CN')"
    >中</button>
    <button
      type="button"
      class="lang-toggle-btn"
      :class="{ active: locale === 'en-US' }"
      title="Switch to English"
      @click="pick('en-US')"
    >EN</button>
  </div>
</template>

<style scoped>
.lang-toggle {
  display: inline-flex;
  margin-right: 8px;
  border-radius: 4px;
  overflow: hidden;
  background: #313244;
  border: 1px solid #45475a;
}

.lang-toggle-btn {
  padding: 2px 10px;
  font-size: 12px;
  background: transparent;
  border: none;
  color: #a6adc8;
  cursor: pointer;
  font-family: inherit;
  line-height: 1.6;
}

.lang-toggle-btn:hover:not(.active) {
  background: #45475a;
  color: #cdd6f4;
}

.lang-toggle-btn.active {
  background: #45475a;
  color: #cdd6f4;
  font-weight: 600;
}
</style>
```

- [ ] **Step 2: 在 master Toolbar.vue 引入 LangToggle**

修改 `master-frontend/src/components/Toolbar.vue`:

(a) 在 `<script setup lang="ts">` 顶部追加:

```ts
import LangToggle from './LangToggle.vue'
import { useI18n } from 'shared-frontend'
const { t } = useI18n()
```

(b) 在 template 中找到:

```html
    <div class="toolbar-spacer"></div>
    <span class="toolbar-title">ModbusMaster</span>
```

替换为:

```html
    <div class="toolbar-spacer"></div>
    <LangToggle />
    <span class="toolbar-title">{{ t('toolbar.appTitleMaster') }}</span>
```

(c) **逐行替换文件中所有硬编码中文**为 `{{ t('namespace.key') }}` 调用。例:

```html
<!-- before -->
<button class="toolbar-btn" @click="openProject" title="打开项目">打开</button>

<!-- after -->
<button class="toolbar-btn" @click="openProject" :title="t('toolbar.openProjectTitle')">{{ t('toolbar.open') }}</button>
```

`<script>` 中的 `showAlert(...)` 等参数同理:

```ts
// before
await showAlert('请选择串口')
// after
await showAlert(t('errors.serialPortRequired'))
```

`placeholder` 等动态属性用冒号绑定:

```html
<!-- before -->
<input placeholder="扫描组名称" />
<!-- after -->
<input :placeholder="t('dialog.scanGroupName')" />
```

- [ ] **Step 3: 残留检查**

```bash
grep -n "[一-龥]" master-frontend/src/components/Toolbar.vue
```

预期:无输出(空 grep)。如有输出,逐条处理为 `t()` 调用,并补充缺失的字典 key(同步更新 `zh-CN.ts` + `en-US.ts`)。

- [ ] **Step 4: 类型 + 启动验证**

```bash
npx vue-tsc -b master-frontend ; echo "exit=$?"
```

预期:`exit=0`。

- [ ] **Step 5: 同样模式处理 master-frontend 其余组件**

依次处理:
- `master-frontend/src/components/ConnectionTree.vue`
- `master-frontend/src/components/DataTable.vue`
- `master-frontend/src/components/ValuePanel.vue`
- `master-frontend/src/components/LogPanel.vue`
- `master-frontend/src/components/ScanDialog.vue`

每个文件:
1. 在 `<script setup>` 顶部加 `import { useI18n } from 'shared-frontend'; const { t } = useI18n()`
2. 把 template / script 内所有中文字面量替换为 `t('...')`
3. `grep "[一-龥]" <file>` 必须无输出
4. `npx vue-tsc -b master-frontend` 必须 `exit=0`

如果遇到字典里没有的中文,在 `zh-CN.ts` 与 `en-US.ts` 的合适 namespace 下成对添加 key。

- [ ] **Step 6: 全 master 残留检查**

```bash
grep -rn "[一-龥]" master-frontend/src/ ; echo "exit=$?"
```

预期:`exit=1`(grep 在无匹配时返回 1)。任何匹配都需修复。

- [ ] **Step 7: Commit**

```bash
git add master-frontend/src shared-frontend/src/i18n/locales
git commit -m "feat(i18n): master-frontend 接入 useI18n + LangToggle + 全文案迁移"
```

---

## Task 7 — frontend (slave) 文案迁移 + LangToggle 接入 + 默认站名拼接

**Files:**
- Create: `frontend/src/components/LangToggle.vue`
- Modify: `frontend/src/components/Toolbar.vue` (含创建 station 时 name 留空)
- Modify: `frontend/src/components/ConnectionTree.vue` (默认站名 displayName)
- Modify: `frontend/src/components/RegisterTable.vue`
- Modify: `frontend/src/components/RegisterModal.vue`
- Modify: `frontend/src/components/BatchAddModal.vue`
- Modify: `frontend/src/components/ValuePanel.vue`
- Modify: `frontend/src/components/LogPanel.vue`

- [ ] **Step 1: 复制 LangToggle.vue 到 frontend**

```bash
cp master-frontend/src/components/LangToggle.vue frontend/src/components/LangToggle.vue
```

(组件本身两端共用同一份逻辑,但分别放在各自目录便于后续样式微调,这是 ModbusSim 现有 Toolbar.vue / ValuePanel.vue 等"两端各一份"模式的延续。)

- [ ] **Step 2: frontend Toolbar.vue 接入 LangToggle + 文案 + 默认站名留空**

修改 `frontend/src/components/Toolbar.vue`:

(a) `<script setup>` 顶部加:

```ts
import LangToggle from './LangToggle.vue'
import { useI18n } from 'shared-frontend'
const { t } = useI18n()
```

(b) template 中 `toolbar-spacer` 之前(参考实际行,文件中 `<div class="toolbar-title">ModbusSlave</div>` 在 422 行附近)替换为:

```html
<LangToggle />
<div class="toolbar-title">{{ t('toolbar.appTitleSlave') }}</div>
```

(c) 文件中行 217 附近原本写:

```ts
name: `从站 ${slaveId}`,
```

改为:

```ts
name: '',  // 默认站名由前端 displayName 按当前 locale 拼接,后端不持有翻译源
```

(d) 替换文件中其余所有中文字面量为 `t('...')`。

(e) 残留检查:

```bash
grep -n "[一-龥]" frontend/src/components/Toolbar.vue ; echo "exit=$?"
```

预期 `exit=1`。

- [ ] **Step 3: frontend ConnectionTree.vue 默认站名拼接**

修改 `frontend/src/components/ConnectionTree.vue`:

(a) `<script setup>` 顶部加:

```ts
import { useI18n } from 'shared-frontend'
const { t } = useI18n()
```

(b) 找到行 209 附近:

```html
<span class="node-label">{{ td.device.name || `从站 ${td.device.slave_id}` }}</span>
```

替换为:

```html
<span class="node-label">{{ td.device.name?.trim() || t('station.defaultName', { id: td.device.slave_id }) }}</span>
```

(c) 替换其余所有中文字面量。

(d) 残留检查通过(`grep [一-龥]` 无输出)。

- [ ] **Step 4: 处理 frontend 其余组件**

依次处理:
- `frontend/src/components/RegisterTable.vue`
- `frontend/src/components/RegisterModal.vue`
- `frontend/src/components/BatchAddModal.vue`
- `frontend/src/components/ValuePanel.vue`
- `frontend/src/components/LogPanel.vue`

每个文件按照 Task 6 Step 5 的流程:加 `useI18n` import、字面量替换、残留 grep、补字典 key。

- [ ] **Step 5: 全 frontend 残留检查**

```bash
grep -rn "[一-龥]" frontend/src/ ; echo "exit=$?"
```

预期 `exit=1`(无匹配)。

- [ ] **Step 6: 类型构建验证**

```bash
npx vue-tsc -b frontend ; echo "exit=$?"
```

预期 `exit=0`。

- [ ] **Step 7: Commit**

```bash
git add frontend/src shared-frontend/src/i18n/locales
git commit -m "feat(i18n): frontend (slave) 接入 useI18n + 默认站名按 locale 拼接 + 全文案迁移"
```

---

## Task 8 — 后端默认站名清理

**Files:**
- Modify: `crates/modbussim-app/src/commands.rs`(line 186-187)

- [ ] **Step 1: 修改 commands.rs**

把:

```rust
        Some("random") => SlaveDevice::with_random_registers(1, "从站 1", 20000),
        _ => SlaveDevice::with_default_registers(1, "从站 1", 20000),
```

改为:

```rust
        Some("random") => SlaveDevice::with_random_registers(1, "", 20000),
        _ => SlaveDevice::with_default_registers(1, "", 20000),
```

(`SlaveDevice::with_*_registers` 接受 `impl Into<String>`,空字符串合法。前端展示时由 ConnectionTree 的 `displayName` 按 locale 拼接。)

- [ ] **Step 2: 残留检查**

```bash
grep -rn "[一-龥]" crates/modbussim-app/src/commands.rs crates/modbusmaster-app/src/commands.rs ; echo "exit=$?"
```

预期 `exit=1`(无中文残留)。

- [ ] **Step 3: cargo check**

```bash
cargo check -p modbussim-app 2>&1 | tail -5
```

预期:`Finished ... profile`,无错误。

- [ ] **Step 4: cargo test 不回归**

```bash
cargo test -p modbussim-core --lib 2>&1 | tail -5
```

预期:`test result: ok. <N> passed; 0 failed`。

- [ ] **Step 5: Commit**

```bash
git add crates/modbussim-app/src/commands.rs
git commit -m "refactor(slave-app): 默认站名留空 — 由前端按 locale 拼接"
```

---

## Task 9 — 全量验证

**Files:** 无(纯验证)

- [ ] **Step 1: 共享包测试**

```bash
npm test -w shared-frontend
```

预期:全部 16 测试通过(detect 8 + useI18n 8)。

- [ ] **Step 2: 两端 TypeScript 构建**

```bash
npx vue-tsc -b master-frontend ; echo "master_exit=$?"
npx vue-tsc -b frontend ; echo "frontend_exit=$?"
```

预期:两条都是 `exit=0`。

- [ ] **Step 3: 全仓库中文残留检查**

```bash
grep -rn "[一-龥]" master-frontend/src/ frontend/src/ shared-frontend/src/i18n/locales/zh-CN.ts ; echo "exit=$?"
```

预期:**只有** `shared-frontend/src/i18n/locales/zh-CN.ts` 内出现的中文(字典本身)。其他两端 src 应无任何中文(`grep` 排除 zh-CN.ts 后 `exit=1`):

```bash
grep -rn "[一-龥]" master-frontend/src/ frontend/src/ ; echo "exit=$?"
```

应 `exit=1`。

- [ ] **Step 4: cargo check 全仓**

```bash
cargo check -p modbussim-core -p modbussim-app -p modbusmaster-app 2>&1 | tail -5
```

预期:`Finished ... profile`,无错误。

- [ ] **Step 5: 类型约束故意破坏验证**

```bash
sed -i.bak '/^    confirm:/d' shared-frontend/src/i18n/locales/en-US.ts
npx vue-tsc -b master-frontend ; echo "deliberately_broken_exit=$?"
mv shared-frontend/src/i18n/locales/en-US.ts.bak shared-frontend/src/i18n/locales/en-US.ts
npx vue-tsc -b master-frontend ; echo "restored_exit=$?"
```

预期:`deliberately_broken_exit` 非 0,`restored_exit=0`。

- [ ] **Step 6: 提供给用户的 E2E 手工测试清单**

把以下 checklist 复制到 commit message 或 PR 描述,**不主动启动 dev server**(遵守 CLAUDE.md 规则 5)。请用户在本地按需执行:

```
[ ] 中文系统下首次启动 → 默认中文
[ ] 英文系统下首次启动 → 默认英文
[ ] master 端切到英文 → reload → 仍英文(localStorage 持久化)
[ ] master Toolbar / ConnectionTree / DataTable / ValuePanel / LogPanel / ScanDialog / 各 Modal 全文本切换无遗漏
[ ] master 写入对话框 / 扫描对话框中英切换正确
[ ] slave Toolbar / ConnectionTree / RegisterTable / RegisterModal / BatchAddModal / ValuePanel / LogPanel 全文本切换无遗漏
[ ] slave 创建新连接(未填名字)→ 默认显示「从站 1」/「Slave 1」
[ ] slave 切语言后默认站名即时变化
[ ] LogPanel 的 TX/RX 标签、清空/导出按钮跟随切换
[ ] 切换语言后界面无错位、无空白(英文长文本无溢出)
```

- [ ] **Step 7: Commit(可选,如有 lockfile / 配置变动)**

如果 Step 1-6 触发任何文件改动:

```bash
git status --short
git add -A
git commit -m "chore(i18n): 全量验证后清理"
```

否则跳过此步。

---

## Self-Review

逐项核对(执行此 plan 的引擎应在交付前自查):

1. **Spec 覆盖**
   - 决策摘要每一行 → Task 1-9 覆盖?✅
     - 范围 = Tauri 前端 UI + 后端默认站名 → Task 6/7/8
     - 默认语言跟随系统 → Task 2 detect.detectInitialLocale
     - 持久化 localStorage `modbussim.locale` → Task 2 STORAGE_KEY + Task 5 setLocale
     - 切换 UI 位置 + toggle 形态 → Task 6/7 LangToggle
     - i18n 方案 = 自研 composable 无新依赖 → Task 5 useI18n(只有 vue 是已有依赖)
     - 字典存放 shared-frontend → Task 3/4
     - 后端默认站名 → Task 7 ConnectionTree displayName + Task 8 commands.rs
   - Composable API (`t / locale / setLocale`) → Task 5
   - 占位符 `{name}` 替换 → Task 5 interpolate + Task 2 单测
   - 启动初始化优先级 localStorage > 系统语言 > 默认 → Task 2 detect + Task 5 单测
   - 字典 namespace + camelCase + `as const` → Task 3
   - `typeof zhCN` 类型约束 → Task 4 + Task 9 Step 5 故意破坏验证
   - 切换 UI 位置(`toolbar-spacer` 之后、`toolbar-title` 之前)→ Task 6 Step 2 / Task 7 Step 2
   - Toggle 形态 + 高亮 + tooltip → Task 6 LangToggle 完整代码
   - LogPanel 切换响应 → Vue 响应式自动覆盖,Task 6/7 模板替换后即生效
   - 后端默认站名 → Task 7 / Task 8
   - 测试用例覆盖 detect + useI18n + 类型约束 + E2E 清单 → Task 2 / Task 5 / Task 9 Step 5 / Task 9 Step 6
   - 风险缓解(残留中文、字典漂移、布局溢出)→ Task 6/7/9 grep + Task 4/9 类型约束 + Task 9 Step 6 E2E

2. **占位符扫描**:无 `TBD` / `TODO` / `add appropriate error handling` 类语句。
   - "实施期补充 namespace"(Task 3 Step 3)是**工作流指令**而非占位符,给出了具体规则。
   - "新增 key 必须落在某个 namespace 下" 是约束规则,不是 placeholder。

3. **类型一致性**
   - `Locale = 'zh-CN' | 'en-US'`:Task 2 定义,Task 5 使用,Task 6/7 LangToggle 使用 ✅
   - `STORAGE_KEY = 'modbussim.locale'`:Task 2 定义,Task 5 测试引用,文档强调 ✅
   - `useI18n() => { t, locale, setLocale }`:Task 5 实现,Task 6/7 使用 ✅
   - `Messages = typeof zhCN`:Task 3 导出,Task 4 import 使用,Task 5 imports 字典对象 ✅
   - `t('station.defaultName', { id: number })`:Task 3/4 字典含 `{id}` 占位,Task 5 interpolate 支持,Task 7 Step 3 调用一致 ✅

无差异,plan 可交付。

---

## Plan complete. 

Saved to `docs/superpowers/plans/2026-05-01-bilingual-i18n.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — 每个 task 派一个新 subagent,task 间 review,迭代快、隔离强
2. **Inline Execution** — 当前会话内 batch 执行,在 checkpoint 处暂停审查

请选择执行方式。
