## 新增需求

### 需求:随机值初始化构造方法

系统必须提供 `SlaveDevice::with_random_registers()` 构造方法，使用随机值初始化寄存器。该方法的签名和行为必须与 `with_default_registers()` 一致（同样创建 FC1/FC2/FC3/FC4 四类寄存器），区别仅在于初始值为随机生成。

#### 场景:随机初始化 Coil 寄存器
- **当** 调用 `with_random_registers(1, "从站 1", 100)`
- **那么** 必须创建 101 个 Coil (FC1) 类型的 RegisterDef，地址从 0 到 100
- **那么** RegisterMap 的 coils 中地址 0~100 必须全部存在，每个值为随机 bool（约 50% 概率 true）

#### 场景:随机初始化 Discrete Input 寄存器
- **当** 调用 `with_random_registers(1, "从站 1", 100)`
- **那么** 必须创建 101 个 Discrete Input (FC2) 类型的 RegisterDef，地址从 0 到 100
- **那么** RegisterMap 的 discrete_inputs 中地址 0~100 必须全部存在，每个值为随机 bool

#### 场景:随机初始化 Holding Register 寄存器
- **当** 调用 `with_random_registers(1, "从站 1", 100)`
- **那么** 必须创建 101 个 Holding Register (FC3) 类型的 RegisterDef，地址从 0 到 100
- **那么** RegisterMap 的 holding_registers 中地址 0~100 必须全部存在，每个值为 0~65535 范围内的随机 u16

#### 场景:随机初始化 Input Register 寄存器
- **当** 调用 `with_random_registers(1, "从站 1", 100)`
- **那么** 必须创建 101 个 Input Register (FC4) 类型的 RegisterDef，地址从 0 到 100
- **那么** RegisterMap 的 input_registers 中地址 0~100 必须全部存在，每个值为 0~65535 范围内的随机 u16

#### 场景:随机初始化生成的寄存器定义数量
- **当** 调用 `with_random_registers(1, "从站 1", 100)`
- **那么** 必须生成 404 个 RegisterDef（4 类 x 101 个地址）

### 需求:创建连接时支持初始值模式选择

`create_slave_connection` 命令必须接受 `init_mode` 参数，支持 `"zero"` 和 `"random"` 两种模式。默认为 `"zero"`。

#### 场景:使用零值模式创建连接
- **当** 用户调用 `create_slave_connection`，`init_mode` 为 `"zero"` 或未指定
- **那么** 自动创建的默认从站必须使用 `with_default_registers()` 初始化，所有值为 0/false

#### 场景:使用随机模式创建连接
- **当** 用户调用 `create_slave_connection`，`init_mode` 为 `"random"`
- **那么** 自动创建的默认从站必须使用 `with_random_registers()` 初始化，值为随机

### 需求:添加从站时支持初始值模式选择

`add_slave_device` 命令必须接受 `init_mode` 参数，支持 `"zero"` 和 `"random"` 两种模式。当 `init_mode` 有值时，必须使用带预填寄存器的构造方法创建从站。

#### 场景:使用随机模式添加从站
- **当** 用户调用 `add_slave_device`，`init_mode` 为 `"random"`
- **那么** 新从站必须使用 `with_random_registers()` 创建，包含预填的随机值寄存器

#### 场景:使用零值模式添加从站
- **当** 用户调用 `add_slave_device`，`init_mode` 为 `"zero"`
- **那么** 新从站必须使用 `with_default_registers()` 创建，包含预填的零值寄存器

### 需求:前端新建从站对话框

前端新建从站的交互必须从简单的 prompt 改为自定义模态框，包含从站 ID、初始值模式选项。

#### 场景:新建从站对话框显示
- **当** 用户点击"新建从站"按钮
- **那么** 必须弹出模态框，包含从站 ID 输入框和初始值模式选项（全零/随机）

#### 场景:选择随机模式新建从站
- **当** 用户在对话框中输入从站 ID 并选择"随机"初始值模式后确认
- **那么** 前端必须调用 `add_slave_device` 命令，传入 `init_mode: "random"`
