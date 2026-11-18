# `/setname` 命令

> 状态：已实现

## 语法

```text
/setname <新名字>
```

## 示例

```text
/setname alice
/setname 张三
/setname alice@arch
```

## 规则

- `/setname` 无参数 → 用法提示，不修改。
- 名字 trim 后为空 → 用法提示。
- 超过 64 字符或 256 字节 → 错误。
- 控制字符 → 错误。
- 允许空格；`/setname alice bob` 的基础名是 `alice bob`。
- 显示时由 `naming::display_name` 追加 `(no1)`；不把短号写进 identity.name。

## 提示

成功：

```text
名称已修改为 alice (no1)
```

失败：

```text
用法：/setname <新名字>
名字太长（最多 64 个字符）
名字不能包含控制字符
保存配置失败: ...
保存身份失败: ...
```

## 与其它命令

- 未知 `/setnameX` 仍是普通消息。
- `/setname` 是已注册命令，参数错误不发送消息。
- Compose 下 `/setname` 不写历史（命令不产生聊天消息）。
