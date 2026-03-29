## 新增需求

### 需求:跨组件寄存器刷新机制
系统必须提供 provide/inject 机制，允许任何组件触发 RegisterTable 重新加载寄存器数据。

#### 场景:ValuePanel 写入后触发刷新
- **当** ValuePanel 成功写入寄存器后调用 refreshRegisters()
- **那么** RegisterTable 必须重新加载所有寄存器值，并通过 emitSelection() 更新 selectedRegister ref

#### 场景:刷新后 ValuePanel 显示更新
- **当** RegisterTable 重新加载完成并 emit 新的 selectedRegister
- **那么** ValuePanel 的所有解析值必须反映最新的寄存器数据
