## 1. 修复表格滚动

- [x] 1.1 在 RegisterTable.vue 中将 `<table>` 包裹在 `<div class="table-scroll-container">` 中，容器设 `overflow-y: auto; flex: 1`
- [x] 1.2 确保 `<thead>` 在滚动容器内保持 `position: sticky; top: 0` 固定
- [x] 1.3 移除 `<table>` 元素上无效的 `flex: 1; overflow-y: auto` 样式
- [x] 1.4 验证 101 个寄存器可完整滚动浏览，表头固定不动

## 2. 新增搜索筛选

- [x] 2.1 在 `table-header-bar` 中添加搜索输入框，绑定 `searchQuery` ref，带占位提示 "搜索地址/名称..."
- [x] 2.2 修改 `filteredRegisters` 计算属性：在现有类型过滤之后叠加搜索条件——`0x` 前缀按十六进制地址匹配，纯数字按十进制地址匹配，其他按名称不区分大小写模糊匹配
- [x] 2.3 搜索框内容变化时清空选中状态
- [x] 2.4 验证搜索与左侧树类型过滤可叠加生效

## 3. 改造选中模型为多选

- [x] 3.1 RegisterTable.vue 中将 `selectedRow: ref<Register | null>` 改为 `selectedRows: ref<Register[]>`，并增加 `lastClickedIndex: ref<number>` 用于 Shift 选择
- [x] 3.2 实现点击行逻辑：普通点击替换选中，Ctrl/Cmd+点击切换选中，Shift+点击范围选
- [x] 3.3 修改行的 `selected` CSS 类判断，从 `selectedRow === reg` 改为 `selectedRows.includes(reg)`
- [x] 3.4 修改 `emit('register-select')` 事件，将选中的所有寄存器（含地址、类型、值）作为数组传出
- [x] 3.5 App.vue 中将 `selectedRegister` 类型从单对象 `ref<{...} | null>` 改为数组 `ref<{...}[]>`，更新 `provide` 和 `handleRegisterSelect`

## 4. 键盘导航

- [x] 4.1 在表格滚动容器上添加 `tabindex="0"` 和 `@keydown` 监听
- [x] 4.2 处理 `ArrowDown`：选中下一行（基于 filteredRegisters 列表），到末尾则不动
- [x] 4.3 处理 `ArrowUp`：选中上一行，到顶部则不动
- [x] 4.4 选中行变化时调用 `scrollIntoView({ block: 'nearest' })` 确保可见

## 5. ValuePanel 多选解析

- [x] 5.1 修改 ValuePanel.vue 中 `selectedRegister` inject 类型为数组
- [x] 5.2 删除自动读取邻居寄存器的 `watch` 逻辑（`neighborValues`）
- [x] 5.3 实现新的解析逻辑：1 个→16 位；2 个同类型→16 位+32 位；4 个同类型→16 位+32 位+64 位；不同类型→只显示各自 16 位
- [x] 5.4 32/64 位解析时使用按地址排序的选中寄存器值
- [x] 5.5 更新面板标题：多选时显示 "4x Holding Register @ 0x0004~0x0007" 格式
- [x] 5.6 Bool 类型寄存器不显示 32/64 位组合解析

## 6. 验证

- [x] 6.1 验证前端编译通过（vue-tsc + vite build）
- [ ] 6.2 手动测试：滚动 101 个寄存器、搜索地址/名称、Ctrl/Shift 多选、键盘导航、ValuePanel 多选解析
