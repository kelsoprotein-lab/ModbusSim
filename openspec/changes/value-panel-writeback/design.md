## 上下文

ValuePanel 当前通过 `inject('selectedRegister')` 获取选中的寄存器数组（地址、类型、值），用 computed 属性做各种位宽的解析展示。RegisterTable 是唯一调用 `invoke('write_register')` 写入后端的组件，写后通过 `emitSelection()` 更新 selectedRegister ref。

现有的跨组件刷新模式：App.vue 中 `treeRefreshKey` + `refreshTree()` 用于通知 ConnectionTree 重新加载。

## 目标 / 非目标

**目标：**
- ValuePanel 中所有解析值支持双击编辑、反向解析、写入寄存器
- 写入后 RegisterTable 自动刷新，ValuePanel 显示更新后的值
- 交互体验与 RegisterTable 的双击编辑一致（Enter 确认、Escape 取消、blur 确认）

**非目标：**
- 不做批量编辑（一次只编辑一个解析字段）
- 不做 Binary 位级别的逐 bit 点击 toggle
- 不做输入验证的实时预览（如边输入边看其他解析值变化）

## 决策

### 1. ValuePanel 直接调用 invoke 写入（方案 A）

**选择**：ValuePanel 自己调用 `invoke('write_register')`，写完后调用 `refreshRegisters()` 通知 RegisterTable 重新加载。

**替代方案**：ValuePanel emit 事件让 RegisterTable 代为写入。

**理由**：ValuePanel 已通过 inject 拥有 `selectedConnectionId`、`selectedSlaveId`，直接 invoke 无需额外事件布线。保持组件自包含，减少 App.vue 的协调复杂度。

### 2. 刷新机制复用 refreshKey 模式

**选择**：App.vue 新增 `registerRefreshKey` ref 和 `refreshRegisters` 函数（`provide`），RegisterTable `watch` 该 key 触发 `loadRegisters()`，ValuePanel 写入后调用 `refreshRegisters()`。

**理由**：与现有 `treeRefreshKey` / `refreshTree` 模式一致，无需引入新概念。

### 3. 编辑状态管理

**选择**：`editingField` ref 记录当前编辑的字段标识（如 `'signed16'`、`'floatABCD'`），同一时间只有一个字段处于编辑状态。

**理由**：ValuePanel 的字段互斥（Signed 和 Float 不会同时编辑），单一状态变量足够。

### 4. 反向解析策略

每种字段类型的反向解析输出为 `Array<{address, register_type, value}>`，统一用同一个写入函数循环调用 invoke。

| 字段 | 输入 | 输出寄存器数 |
|------|------|-------------|
| Signed/Unsigned/Hex 16 | number/hex string | 1 (sortedRegs[0]) |
| Long AB CD / CD AB | uint32 | 2 (sortedRegs[0..1]) |
| Float AB CD / CD AB | float | 2 (sortedRegs[0..1]) |
| Double | float64 | 4 (sortedRegs[0..3]) |
| Bool | toggle | 1 |

## 风险 / 权衡

- **多寄存器写入非原子**：32-bit Float 需要写 2 个寄存器，两次 invoke 之间有短暂不一致窗口。对于模拟器场景可接受，无需事务支持。
- **编辑中选区变化**：用户正在编辑 Float 时切换了选中的寄存器，需要取消当前编辑。通过 watch selectedRegister 变化时重置 editingField 解决。
