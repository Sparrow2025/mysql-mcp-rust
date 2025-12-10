# Implementation Plan

- [x] 1. 设置项目结构和核心依赖
  - 创建 Rust 项目，配置 Cargo.toml 添加必要依赖（rmcp, sqlx, tokio, serde, tracing）
  - 设置项目目录结构（src/config, src/manager, src/tools, src/resources, src/error）
  - 配置日志和追踪系统
  - _Requirements: 14.1_

- [x] 2. 实现配置管理模块
  - 定义配置数据结构（DataSourceConfig, PoolConfig, ServerConfig）
  - 实现从 TOML/YAML 文件加载配置
  - 实现从环境变量加载敏感信息（密码）
  - 实现配置验证逻辑（检查必填字段）
  - _Requirements: 1.1, 1.2, 8.1_

- [x] 2.1 编写配置验证的属性测试
  - **Property 1: Configuration validation completeness**
  - **Validates: Requirements 1.2**

- [x] 2.2 编写无效配置处理的属性测试
  - **Property 2: Invalid configuration handling**
  - **Validates: Requirements 1.3**

- [x] 3. 实现数据源管理器
  - 实现 DataSourceManager 结构体
  - 实现数据源密钥生成和映射逻辑
  - 实现数据源查找和验证方法
  - 实现数据源列表功能（不暴露凭证）
  - _Requirements: 8.2, 8.3, 8.5, 6.1_

- [x] 3.1 编写数据源密钥唯一性的属性测试
  - **Property 18: Data source key uniqueness**
  - **Validates: Requirements 8.2**

- [x] 3.2 编写密钥映射正确性的属性测试
  - **Property 20: Key-to-credentials mapping correctness**
  - **Validates: Requirements 8.5**

- [x] 4. 实现连接池管理器
  - 使用 sqlx::Pool 为每个数据源创建连接池
  - 实现 ConnectionPoolManager 结构体
  - 配置连接池参数（最大连接数、超时、空闲时间）
  - 实现连接健康检查逻辑
  - 实现连接池统计信息收集
  - _Requirements: 1.4, 2.5, 5.1, 5.2, 12.3, 12.4_

- [x] 4.1 编写连接池创建一致性的属性测试
  - **Property 3: Pool creation consistency**
  - **Validates: Requirements 1.4**

- [x] 4.2 编写连接池隔离的属性测试
  - **Property 6: Connection pool isolation**
  - **Validates: Requirements 2.5**

- [x] 5. 实现错误处理和重试机制
  - 定义 McpError 枚举类型
  - 实现错误消息脱敏逻辑（移除凭证信息）
  - 实现连接失败重试逻辑（指数退避，最多 3 次）
  - 实现数据源状态管理（available/unavailable）
  - 实现后台重连任务（每 60 秒）
  - _Requirements: 7.1, 7.2, 7.3, 7.5, 10.2_

- [x] 5.1 编写错误消息脱敏的属性测试
  - **Property 23: Credential non-disclosure**
  - **Validates: Requirements 10.1, 10.2, 10.3, 10.5**

- [x] 6. 实现查询工具 (mysql_query)
  - 实现 QueryTool 结构体
  - 实现参数验证（数据源密钥、数据库名称、SQL 查询）
  - 实现查询执行逻辑（使用 sqlx）
  - 实现查询超时机制（30 秒）
  - 实现多语句查询处理（只执行第一条）
  - 实现结果格式化（列元数据 + 行数据）
  - _Requirements: 2.3, 3.1, 3.2, 3.4, 3.5_

- [x] 6.1 编写查询参数验证的属性测试
  - **Property 4: Query parameter validation**
  - **Validates: Requirements 2.3**

- [x] 6.2 编写查询执行正确性的属性测试
  - **Property 7: Query execution correctness**
  - **Validates: Requirements 3.1**

- [x] 6.3 编写结果格式一致性的属性测试
  - **Property 8: Result format consistency**
  - **Validates: Requirements 3.2**

- [x] 6.4 编写多语句查询处理的属性测试
  - **Property 10: Multi-statement query handling**
  - **Validates: Requirements 3.4**

- [x] 7. 实现流式查询支持
  - 实现 QueryResultStream 结构体
  - 实现分块发送逻辑（每次最多 1000 行）
  - 实现流取消和资源清理
  - 实现并发流隔离
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

- [x] 7.1 编写流分块大小限制的属性测试
  - **Property 21: Stream chunk size limit**
  - **Validates: Requirements 9.2**

- [x] 7.2 编写并发流隔离的属性测试
  - **Property 22: Concurrent stream isolation**
  - **Validates: Requirements 9.5**

- [x] 8. 实现执行工具 (mysql_execute)
  - 实现 ExecuteTool 结构体
  - 实现 DML 语句执行（INSERT, UPDATE, DELETE）
  - 实现 DDL 语句拒绝逻辑
  - 实现返回受影响行数
  - 实现返回 last_insert_id
  - 实现自动事务提交
  - _Requirements: 11.1, 11.2, 11.3, 11.5_

- [x] 8.1 编写 DML 执行正确性的属性测试
  - **Property 24: DML execution correctness**
  - **Validates: Requirements 11.1**

- [x] 8.2 编写 DDL 拒绝的属性测试
  - **Property 26: DDL statement rejection**
  - **Validates: Requirements 11.3**

- [x] 9. 实现 Schema 工具
  - 实现 SchemaTool 结构体
  - 实现 mysql_list_tables 工具（列出所有表）
  - 实现 mysql_describe_table 工具（获取表结构）
  - 实现获取主键、外键、索引信息
  - 实现非存在表的错误处理
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

- [x] 9.1 编写表列表完整性的属性测试
  - **Property 11: Table listing completeness**
  - **Validates: Requirements 4.1**

- [x] 9.2 编写表结构完整性的属性测试
  - **Property 12: Table schema completeness**
  - **Validates: Requirements 4.2, 4.4, 4.5**

- [x] 9.3 编写非存在表错误处理的属性测试
  - **Property 13: Non-existent table error handling**
  - **Validates: Requirements 4.3**

- [x] 10. 实现列表工具
  - 实现 mysql_list_datasources 工具
  - 实现 mysql_list_databases 工具
  - 实现数据库元数据获取（大小、字符集）
  - 实现数据库列表缓存（60 秒）
  - 实现无效数据源密钥的错误处理
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [x] 10.1 编写数据源列表准确性的属性测试
  - **Property 14: Data source listing accuracy**
  - **Validates: Requirements 6.1**

- [x] 10.2 编写数据库列表准确性的属性测试
  - **Property 15: Database listing accuracy**
  - **Validates: Requirements 6.2, 6.4**

- [x] 10.3 编写无效密钥处理的属性测试
  - **Property 16: Invalid data source key handling**
  - **Validates: Requirements 6.3, 8.4**

- [x] 11. 实现连接统计工具 (mysql_get_connection_stats)
  - 实现 StatsTool 结构体
  - 实现获取单个数据源的统计信息
  - 实现获取所有数据源的统计信息
  - 实现实时统计（不缓存）
  - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5_

- [x] 11.1 编写连接统计完整性的属性测试
  - **Property 28: Connection stats completeness**
  - **Validates: Requirements 12.3, 12.4**

- [x] 12. 实现 MCP Resources 接口
  - 实现 ResourceProvider trait
  - 实现资源 URI 解析和验证
  - 实现 mysql://datasources 资源
  - 实现 mysql://{key}/databases 资源
  - 实现 mysql://{key}/{db}/tables 资源
  - 实现 mysql://{key}/{db}/tables/{table} 资源
  - 实现 mysql://{key}/{db}/schema 资源
  - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_

- [x] 12.1 编写资源 URI 验证的属性测试
  - **Property 29: Resource URI validation**
  - **Validates: Requirements 13.1**

- [x] 12.2 编写资源内容正确性的属性测试
  - **Property 30: Resource content correctness**
  - **Validates: Requirements 13.2, 13.3, 13.4, 13.5**

- [x] 13. 使用 rmcp 集成 MCP 服务器
  - 使用 rmcp::ServerBuilder 创建 MCP 服务器
  - 注册所有工具（7 个工具）
  - 注册资源提供者
  - 配置服务器信息（名称、版本、协议版本）
  - 配置服务器能力（tools, resources）
  - 实现 stdio 传输层
  - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5_

- [x] 13.1 编写 MCP 协议合规性的属性测试
  - **Property 31: MCP protocol compliance**
  - **Validates: Requirements 14.2, 14.3, 14.4, 14.5**

- [x] 14. 实现日志和监控
  - 配置 tracing 订阅者
  - 实现敏感信息脱敏的日志过滤器
  - 实现连接池统计定期日志（每 60 秒）
  - 实现结构化日志（包含 trace ID）
  - 实现不同日志级别的输出
  - _Requirements: 5.5, 10.3_

- [x] 15. 创建配置文件示例和文档
  - 创建示例配置文件（config.example.toml）
  - 编写配置文件格式说明
  - 编写环境变量使用说明
  - 创建 README.md 文档
  - _Requirements: 1.1, 8.1_

- [x] 16. 实现主程序入口
  - 实现 main.rs 入口点
  - 实现配置加载逻辑
  - 实现服务器启动和优雅关闭
  - 实现信号处理（SIGTERM, SIGINT）
  - 实现连接池清理逻辑
  - _Requirements: 5.4_

- [x] 17. Checkpoint - 确保所有测试通过
  - 确保所有测试通过，如有问题请询问用户

- [x] 18. 端到端集成测试
  - 使用 Docker 启动测试 MySQL 实例
  - 测试完整的查询流程
  - 测试多数据源并发访问
  - 测试错误恢复场景
  - 测试流式查询
  - _Requirements: 所有需求_

- [x] 19. 性能优化和最终调整
  - 优化连接池配置
  - 优化缓存策略
  - 优化内存使用
  - 进行性能基准测试
  - _Requirements: 1.5_
