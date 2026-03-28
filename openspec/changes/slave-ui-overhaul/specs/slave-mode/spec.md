## 修改需求

### 需求:创建从站连接

系统必须允许用户通过指定端口号创建 Modbus TCP 从站连接。创建连接时必须自动创建一个 slave_id=1 的默认从站设备，预填 FC1/FC2/FC3/FC4 四类寄存器（地址 0~100），并初始化 RegisterMap 中的对应值。返回的 SlaveConnectionInfo 中 device_count 必须为 1。

#### 场景:创建连接并验证默认从站
- **当** 用户调用 create_slave_connection 命令，端口为 5020
- **那么** 系统必须创建连接、自动添加默认从站（slave_id=1）、预填 404 个 RegisterDef、初始化 RegisterMap
- **那么** 返回的 SlaveConnectionInfo.device_count 必须为 1

#### 场景:创建连接后列出设备
- **当** 用户调用 create_slave_connection 后立即调用 list_slave_devices
- **那么** 必须返回包含一个 SlaveDeviceInfo 的列表，slave_id=1，register_count=404

#### 场景:创建连接后启动并响应请求
- **当** 用户创建连接并启动后，Modbus 主站发送 FC03 ReadHoldingRegisters(0, 10) 请求
- **那么** 从站必须返回 10 个值为 0 的有效响应
