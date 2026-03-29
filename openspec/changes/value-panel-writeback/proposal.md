## 为什么

ValuePanel 当前是纯只读的值解析面板。用户选中寄存器后可以看到 16/32/64 位的各种解析（Signed、Unsigned、Hex、Float 等），但无法直接在解析视图中编辑值并写回寄存器。如果用户想设置一个 Float32 值（如 3.14），必须手动计算出对应的两个 16-bit 寄存器原始值，再到 RegisterTable 中逐个修改——这对模拟器的日常使用体验是一个明显的短板。

## 变更内容

- **新增 ValuePanel 值回写能力**：用户可以双击 ValuePanel 中的解析值进入编辑模式，输入目标值后自动反向解析并写入对应的寄存器
- 支持所有解析类型的回写：16-bit (Signed/Unsigned/Hex)、32-bit (Long/Float AB CD 和 CD AB)、64-bit (Double)、Bool (toggle)
- 写入完成后自动触发 RegisterTable 刷新，保持数据一致性
- 新增 `registerRefreshKey` provide/inject 机制，实现 ValuePanel → RegisterTable 的刷新通知

## 功能 (Capabilities)

### 新增功能
- `value-writeback`: ValuePanel 中双击解析值进行编辑，反向解析后写入寄存器，涵盖所有数值类型的双向转换逻辑
- `register-refresh`: 跨组件寄存器刷新机制，ValuePanel 写入后通知 RegisterTable 重新加载数据

### 修改功能

## 影响

- `frontend/src/components/ValuePanel.vue`: 从只读展示组件变为可交互编辑组件，新增 invoke 调用和编辑状态管理
- `frontend/src/components/RegisterTable.vue`: 新增 watch registerRefreshKey 触发重新加载
- `frontend/src/App.vue`: 新增 registerRefreshKey / refreshRegisters 的 provide
