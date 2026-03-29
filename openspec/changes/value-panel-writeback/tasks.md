## 1. 刷新机制

- [x] 1.1 App.vue 中新增 `registerRefreshKey` ref 和 `refreshRegisters` 函数，通过 provide 暴露
- [x] 1.2 RegisterTable.vue 中 inject `registerRefreshKey`，watch 变化时调用 `loadRegisters()` 并在加载完成后 `emitSelection()`

## 2. ValuePanel 编辑状态管理

- [x] 2.1 新增 `editingField` ref（字段标识如 `'signed16'`/`'floatABCD'` 或 null）和 `editValue` ref
- [x] 2.2 实现 `startEdit(field)` 函数：设置 editingField，预填当前解析值到 editValue
- [x] 2.3 实现 `cancelEdit()` 函数：重置 editingField 为 null
- [x] 2.4 watch selectedRegister 变化时调用 cancelEdit()

## 3. 反向解析逻辑

- [x] 3.1 实现 16-bit 反向解析：Signed（负值 +0x10000）、Unsigned（& 0xFFFF）、Hex（parseInt hex）→ 返回单寄存器写入数组
- [x] 3.2 实现 32-bit 反向解析：Long AB CD / CD AB（移位拆分）、Float AB CD / CD AB（DataView setFloat32 → 两个 getUint16）→ 返回双寄存器写入数组
- [x] 3.3 实现 64-bit 反向解析：Double（DataView setFloat64 → 四个 getUint16）→ 返回四寄存器写入数组
- [x] 3.4 实现 Bool toggle 逻辑：0→1 / 1→0

## 4. 写入与刷新

- [x] 4.1 实现 `commitEdit()` 函数：根据 editingField 调用对应的反向解析，循环 invoke('write_register') 写入每个寄存器，成功后调用 refreshRegisters()，处理无效输入（NaN 时静默退出）
- [x] 4.2 实现键盘/焦点事件绑定：Enter → commitEdit，Escape → cancelEdit，blur → commitEdit

## 5. 模板改造

- [x] 5.1 将每个解析值的 `<span>` 改为支持双击的交互元素：非编辑态显示值文本（@dblclick 触发 startEdit），编辑态显示 `<input>`（autofocus, @keydown, @blur）
- [x] 5.2 Bool 类型行改为双击直接 toggle（不显示输入框），调用 invoke 写入后 refreshRegisters()
- [x] 5.3 值不可用时（显示 "-"）禁止双击进入编辑
