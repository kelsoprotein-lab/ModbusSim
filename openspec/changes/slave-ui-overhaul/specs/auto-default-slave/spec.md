## 新增需求

### 需求:创建连接时自动生成默认从站

系统在创建新的从站连接时，必须自动创建一个 slave_id=1 的默认从站设备，并预填四类寄存器定义。

#### 场景:创建连接后自动包含从站
- **当** 用户创建一个端口为 5020 的从站连接
- **那么** 该连接下必须自动存在一个 slave_id=1、名称为"从站 1"的从站设备
- **那么** 返回的连接信息中 device_count 必须为 1

### 需求:默认从站预填寄存器定义

自动创建的默认从站必须包含 FC1/FC2/FC3/FC4 四类寄存器，每类地址范围 0~100（共 101 个地址）。

#### 场景:预填 Coil 寄存器
- **当** 默认从站创建完成
- **那么** 从站必须包含 101 个 Coil (FC1) 类型的 RegisterDef，地址从 0 到 100，数据类型为 Bool，默认值为 false

#### 场景:预填 Discrete Input 寄存器
- **当** 默认从站创建完成
- **那么** 从站必须包含 101 个 Discrete Input (FC2) 类型的 RegisterDef，地址从 0 到 100，数据类型为 Bool，默认值为 false

#### 场景:预填 Holding Register 寄存器
- **当** 默认从站创建完成
- **那么** 从站必须包含 101 个 Holding Register (FC3) 类型的 RegisterDef，地址从 0 到 100，数据类型为 UInt16，默认值为 0

#### 场景:预填 Input Register 寄存器
- **当** 默认从站创建完成
- **那么** 从站必须包含 101 个 Input Register (FC4) 类型的 RegisterDef，地址从 0 到 100，数据类型为 UInt16，默认值为 0

### 需求:默认寄存器的值初始化

除了 RegisterDef 元数据，RegisterMap 中对应地址的实际值也必须初始化。

#### 场景:Coil 值初始化
- **当** 默认从站创建完成
- **那么** RegisterMap 的 coils 中地址 0~100 必须全部存在，值为 false

#### 场景:Holding Register 值初始化
- **当** 默认从站创建完成
- **那么** RegisterMap 的 holding_registers 中地址 0~100 必须全部存在，值为 0

#### 场景:读取默认寄存器值
- **当** Modbus 主站向默认从站发送 FC03 ReadHoldingRegisters(0, 10) 请求
- **那么** 从站必须返回 10 个值为 0 的寄存器数据，无异常
