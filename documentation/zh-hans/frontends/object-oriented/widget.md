# 界面组件类型 (Widget)

界面组件类型是 Valkyrie 中专门为 UI 开发设计的特殊类类型。它提供了构建现代用户界面的高级抽象，支持响应式设计、状态管理和事件处理。

## 基本组件定义

### 简单组件

```valkyrie
# 基本按钮组件
widget Button {
    # 组件属性
    text: string,
    enabled: bool,
    style: ButtonStyle,
    
    # 事件处理器
    on_click: (micro() -> unit)?,
    
    # 构造函数
    micro constructor(self, text: string) {
        self.text = text
        self.enabled = true
        self.style = ButtonStyle::default()
        self.on_click = None
    }
    
    # 渲染方法
    micro render(self) -> Element {
        Element::button()
            .text(self.text)
            .enabled(self.enabled)
            .style(self.style)
            .on_click(self.on_click)
    }
    
    # 设置点击事件
    micro on_click(mut self, handler: micro() -> unit) -> Self {
        self.on_click = handler
        self
    }
    
    # 设置样式
    micro with_style(mut self, style: ButtonStyle) -> Self {
        self.style = style
        self
    }
}
```

### 文本输入组件

```valkyrie
widget TextInput {
    value: string,
    placeholder: string,
    max_length: usize?,
    readonly: bool,
    
    # 事件处理器
    on_change: (micro(string) -> unit)?,
    on_focus: (micro() -> unit)?,
    on_blur: (micro() -> unit)?,
    
    micro constructor(self, placeholder: string = "") {
        self.value = ""
        self.placeholder = placeholder
        self.max_length = None
        self.readonly = false
        self.on_change = None
        self.on_focus = None
        self.on_blur = None
    }
    
    micro render(self) -> Element {
        Element::input()
            .value(self.value)
            .placeholder(self.placeholder)
            .max_length(self.max_length)
            .readonly(self.readonly)
            .on_change(self.on_change)
            .on_focus(self.on_focus)
            .on_blur(self.on_blur)
    }
    
    # 设置值
    micro set_value(mut self, value: string) {
        match self.max_length {
            case max_len:
                if value.len() > max_len {
                    return
                }
            case None: {}
        }
        
        self.value = value
        
        match self.on_change {
            case handler: handler(self.value.clone())
            case None: {}
        }
    }
    
    # 链式配置
    max_length(mut self, length: usize) -> Self {
        self.max_length = Some(length)
        self
    }
    
    readonly(mut self, readonly: bool = true) -> Self {
        self.readonly = readonly
        self
    }
}
```

## 布局组件

### 容器组件

```valkyrie
widget Container {
    children: [Box<dyn Widget>],
    padding: Padding,
    margin: Margin,
    background: Option<Color>,
    border: Option<Border>,
    
    new() {
        self.children = []
        self.padding = Padding::zero()
        self.margin = Margin::zero()
        self.background = None
        self.border = None
    }
    
    render(self) -> Element {
        let mut element = Element::div()
            .padding(self.padding)
            .margin(self.margin)
        
        if let Some(bg) = self.background {
            element = element.background(bg)
        }
        
        if let Some(border) = self.border {
            element = element.border(border)
        }
        
        for child in self.children {
            element = element.child(child.render())
        }
        
        element
    }
    
    # 添加子组件
    add_child(mut self, child: Box<dyn Widget>) -> Self {
        self.children.push(child)
        self
    }
    
    # 批量添加子组件
    add_children(mut self, children: [Box<dyn Widget>]) -> Self {
        self.children.extend(children)
        self
    }
    
    # 设置样式
    padding(mut self, padding: Padding) -> Self {
        self.padding = padding
        self
    }
    
    background(mut self, color: Color) -> Self {
        self.background = Some(color)
        self
    }
}
```

### 弹性布局

```valkyrie
widget FlexBox {
    children: [FlexChild],
    direction: FlexDirection,
    justify_content: JustifyContent,
    align_items: AlignItems,
    wrap: FlexWrap,
    gap: f32,
    
    new(direction: FlexDirection = FlexDirection::Row) {
        self.children = []
        self.direction = direction
        self.justify_content = JustifyContent::Start
        self.align_items = AlignItems::Stretch
        self.wrap = FlexWrap::NoWrap
        self.gap = 0.0
    }
    
    render(self) -> Element {
        let mut element = Element::div()
            .display(Display::Flex)
            .flex_direction(self.direction)
            .justify_content(self.justify_content)
            .align_items(self.align_items)
            .flex_wrap(self.wrap)
            .gap(self.gap)
        
        for child in self.children {
            let child_element = child.widget.render()
                .flex_grow(child.grow)
                .flex_shrink(child.shrink)
                .flex_basis(child.basis)
            
            element = element.child(child_element)
        }
        
        element
    }
    
    # 添加弹性子项
    add_flex_child(mut self, widget: Box<dyn Widget>, grow: f32 = 0.0, shrink: f32 = 1.0, basis: FlexBasis = FlexBasis::Auto) -> Self {
        self.children.push(FlexChild {
            widget,
            grow,
            shrink,
            basis,
        })
        self
    }
    
    # 设置布局属性
    justify_content(mut self, justify: JustifyContent) -> Self {
        self.justify_content = justify
        self
    }
    
    align_items(mut self, align: AlignItems) -> Self {
        self.align_items = align
        self
    }
}
```

### 网格布局

```valkyrie
widget Grid {
    children: [GridChild],
    template_columns: [GridTrack],
    template_rows: [GridTrack],
    gap: GridGap,
    
    new(columns: [GridTrack], rows: [GridTrack]) {
        self.children = []
        self.template_columns = columns
        self.template_rows = rows
        self.gap = GridGap::zero()
    }
    
    render(self) -> Element {
        let mut element = Element::div()
            .display(Display::Grid)
            .grid_template_columns(self.template_columns)
            .grid_template_rows(self.template_rows)
            .gap(self.gap)
        
        for child in self.children {
            let child_element = child.widget.render()
                .grid_column(child.column)
                .grid_row(child.row)
            
            element = element.child(child_element)
        }
        
        element
    }
    
    # 添加网格子项
    add_grid_child(mut self, widget: Box<dyn Widget>, column: GridPosition, row: GridPosition) -> Self {
        self.children.push(GridChild {
            widget,
            column,
            row,
        })
        self
    }
}
```

## 状态管理组件

### 有状态组件

```valkyrie
widget Counter {
    count: i32,
    step: i32,
    min_value: Option<i32>,
    max_value: Option<i32>,
    
    # 事件处理器
    on_change: Option<micro(i32) -> ()>,
    
    new(initial_count: i32 = 0, step: i32 = 1) {
        self.count = initial_count
        self.step = step
        self.min_value = None
        self.max_value = None
        self.on_change = None
    }
    
    render(self) -> Element {
        FlexBox::new(FlexDirection::Row)
            .add_flex_child(
                Box::new(Button::new("-")
                    .on_click { self.decrement() }),
                0.0, 1.0, FlexBasis::Auto
            )
            .add_flex_child(
                Box::new(Text::new(self.count.to_string())
                    .align(TextAlign::Center)),
                1.0, 1.0, FlexBasis::Auto
            )
            .add_flex_child(
                Box::new(Button::new("+")
                    .on_click { self.increment() }),
                0.0, 1.0, FlexBasis::Auto
            )
            .render()
    }
    
    # 增加计数
    increment(mut self) {
        let new_count = self.count + self.step
        
        if let Some(max) = self.max_value {
            if new_count > max {
                return
            }
        }
        
        self.count = new_count
        self.notify_change()
    }
    
    # 减少计数
    decrement(mut self) {
        let new_count = self.count - self.step
        
        if let Some(min) = self.min_value {
            if new_count < min {
                return
            }
        }
        
        self.count = new_count
        self.notify_change()
    }
    
    # 通知变化
    notify_change(self) {
        if let Some(handler) = self.on_change {
            handler(self.count)
        }
    }
    
    # 设置范围
    range(mut self, min: i32, max: i32) -> Self {
        self.min_value = Some(min)
        self.max_value = Some(max)
        self
    }
}
```

### 表单组件

```valkyrie
widget Form<T> {
    fields: {String: Box<dyn Widget>},
    validators: {String: [Validator]},
    data: T,
    errors: {String: [String]},
    
    # 事件处理器
    on_submit: Option<micro(T) -> ()>,
    on_validate: Option<micro(ValidationResult) -> ()>,
    
    new(initial_data: T) {
        self.fields = {}
        self.validators = {}
        self.data = initial_data
        self.errors = {}
        self.on_submit = None
        self.on_validate = None
    }
    
    render(self) -> Element {
        let mut form_element = Element::form()
            .on_submit { $e -> 
                $e.prevent_default()
                self.handle_submit()
            }
        
        # 渲染字段
        for (name, field) in self.fields {
            let field_container = Container::new()
                .add_child(field)
            
            # 添加错误信息
            if let Some(field_errors) = self.errors.get(name) {
                for error in field_errors {
                    field_container = field_container.add_child(
                        Box::new(Text::new(error)
                            .color(Color::Red)
                            .size(TextSize::Small))
                    )
                }
            }
            
            form_element = form_element.child(field_container.render())
        }
        
        # 提交按钮
        form_element = form_element.child(
            Button::new("提交")
                .type_(ButtonType::Submit)
                .render()
        )
        
        form_element
    }
    
    # 添加字段
    add_field(mut self, name: String, field: Box<dyn Widget>) -> Self {
        self.fields.insert(name, field)
        self
    }
    
    # 添加验证器
    add_validator(mut self, field_name: String, validator: Validator) -> Self {
        if !self.validators.contains_key(field_name) {
            self.validators.insert(field_name.clone(), [])
        }
        self.validators[field_name].push(validator)
        self
    }
    
    # 验证表单
    validate(mut self) -> ValidationResult {
        self.errors.clear()
        let mut is_valid = true
        
        for (field_name, validators) in self.validators {
            let field_value = self.get_field_value(field_name)
            
            for validator in validators {
                if let Err(error) = validator.validate(field_value) {
                    if !self.errors.contains_key(field_name) {
                        self.errors.insert(field_name.clone(), [])
                    }
                    self.errors[field_name].push(error)
                    is_valid = false
                }
            }
        }
        
        let result = ValidationResult { is_valid, errors: self.errors.clone() }
        
        if let Some(handler) = self.on_validate {
            handler(result.clone())
        }
        
        result
    }
    
    # 处理提交
    handle_submit(mut self) {
        let validation_result = self.validate()
        
        if validation_result.is_valid {
            if let Some(handler) = self.on_submit {
                handler(self.data.clone())
            }
        }
    }
}
```

## 高级组件

### 虚拟滚动列表

```valkyrie
widget VirtualList<T> {
    items: [T],
    item_height: f32,
    container_height: f32,
    scroll_top: f32,
    
    # 渲染函数
    render_item: micro(T, usize) -> Box<dyn Widget>,
    
    new(items: [T], item_height: f32, container_height: f32) {
        self.items = items
        self.item_height = item_height
        self.container_height = container_height
        self.scroll_top = 0.0
        self.render_item = micro(item, index) { 
            Box::new(Text::new(format!("Item ${index}")))
        }
    }
    
    render(self) -> Element {
        let visible_start = (self.scroll_top / self.item_height).floor() as usize
        let visible_count = (self.container_height / self.item_height).ceil() as usize + 1
        let visible_end = (visible_start + visible_count).min(self.items.len())
        
        let mut container = Container::new()
            .height(self.container_height)
            .overflow_y(Overflow::Scroll)
            .on_scroll { $e -> self.handle_scroll($e.scroll_top) }
        
        # 上方占位符
        if visible_start > 0 {
            let spacer_height = visible_start as f32 * self.item_height
            container = container.add_child(
                Box::new(Spacer::new().height(spacer_height))
            )
        }
        
        # 可见项目
        for i in visible_start..visible_end {
            let item = &self.items[i]
            let item_widget = (self.render_item)(item.clone(), i)
            container = container.add_child(item_widget)
        }
        
        # 下方占位符
        if visible_end < self.items.len() {
            let spacer_height = (self.items.len() - visible_end) as f32 * self.item_height
            container = container.add_child(
                Box::new(Spacer::new().height(spacer_height))
            )
        }
        
        container.render()
    }
    
    # 处理滚动
    handle_scroll(mut self, scroll_top: f32) {
        self.scroll_top = scroll_top
        # 触发重新渲染
        self.request_update()
    }
    
    # 设置项目渲染器
    item_renderer(mut self, renderer: micro(T, usize) -> Box<dyn Widget>) -> Self {
        self.render_item = renderer
        self
    }
}
```

### 模态对话框

```valkyrie
widget Modal {
    visible: bool,
    title: String,
    content: Box<dyn Widget>,
    closable: bool,
    
    # 事件处理器
    on_close: Option<micro() -> ()>,
    on_confirm: Option<micro() -> ()>,
    
    new(title: String, content: Box<dyn Widget>) {
        self.visible = false
        self.title = title
        self.content = content
        self.closable = true
        self.on_close = None
        self.on_confirm = None
    }
    
    render(self) -> Element {
        if !self.visible {
            return Element::empty()
        }
        
        # 遮罩层
        let overlay = Element::div()
            .position(Position::Fixed)
            .top(0)
            .left(0)
            .width("100%")
            .height("100%")
            .background(Color::rgba(0, 0, 0, 0.5))
            .z_index(1000)
            .on_click { 
                if self.closable {
                    self.close()
                }
            }
        
        # 对话框内容
        let dialog = Container::new()
            .background(Color::White)
            .border_radius(8.0)
            .padding(Padding::all(20.0))
            .max_width(500.0)
            .position(Position::Relative)
            .on_click { $e -> $e.stop_propagation() }
        
        # 标题栏
        let header = FlexBox::new(FlexDirection::Row)
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Center)
            .add_flex_child(
                Box::new(Text::new(self.title)
                    .size(TextSize::Large)
                    .weight(FontWeight::Bold)),
                1.0, 1.0, FlexBasis::Auto
            )
        
        if self.closable {
            header = header.add_flex_child(
                Box::new(Button::new("×")
                    .variant(ButtonVariant::Ghost)
                    .on_click { self.close() }),
                0.0, 1.0, FlexBasis::Auto
            )
        }
        
        dialog = dialog
            .add_child(Box::new(header))
            .add_child(self.content.clone())
        
        # 居中显示
        let centered = FlexBox::new(FlexDirection::Column)
            .justify_content(JustifyContent::Center)
            .align_items(AlignItems::Center)
            .width("100%")
            .height("100%")
            .add_flex_child(Box::new(dialog), 0.0, 1.0, FlexBasis::Auto)
        
        overlay.child(centered.render())
    }
    
    # 显示对话框
    show(mut self) {
        self.visible = true
        self.request_update()
    }
    
    # 关闭对话框
    close(mut self) {
        self.visible = false
        
        if let Some(handler) = self.on_close {
            handler()
        }
        
        self.request_update()
    }
}
```

## 响应式设计

### 媒体查询组件

```valkyrie
widget Responsive {
    breakpoints: {String: f32},
    current_breakpoint: String,
    children: {String: Box<dyn Widget>},
    
    new() {
        self.breakpoints = {
            "mobile": 768.0,
            "tablet": 1024.0,
            "desktop": 1200.0,
        }
        self.current_breakpoint = "desktop"
        self.children = {}
        
        # 监听窗口大小变化
        self.setup_resize_listener()
    }
    
    render(self) -> Element {
        if let Some(child) = self.children.get(self.current_breakpoint) {
            child.render()
        } else {
            Element::empty()
        }
    }
    
    # 为不同断点设置组件
    for_breakpoint(mut self, breakpoint: String, widget: Box<dyn Widget>) -> Self {
        self.children.insert(breakpoint, widget)
        self
    }
    
    # 设置断点
    breakpoint(mut self, name: String, width: f32) -> Self {
        self.breakpoints.insert(name, width)
        self
    }
    
    # 更新当前断点
    update_breakpoint(mut self, window_width: f32) {
        let mut current = "mobile"
        
        for (name, width) in self.breakpoints {
            if window_width >= width {
                current = name
            }
        }
        
        if current != self.current_breakpoint {
            self.current_breakpoint = current
            self.request_update()
        }
    }
}
```

## 动画和过渡

### 动画组件

```valkyrie
widget Animated {
    child: Box<dyn Widget>,
    animation: Animation,
    duration: Duration,
    easing: EasingFunction,
    
    new(child: Box<dyn Widget>, animation: Animation) {
        self.child = child
        self.animation = animation
        self.duration = Duration::milliseconds(300)
        self.easing = EasingFunction::EaseInOut
    }
    
    render(self) -> Element {
        self.child.render()
            .animate(self.animation)
            .duration(self.duration)
            .easing(self.easing)
    }
    
    # 设置动画属性
    duration(mut self, duration: Duration) -> Self {
        self.duration = duration
        self
    }
    
    easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing
        self
    }
}

# 预定义动画
enum Animation {
    FadeIn,
    FadeOut,
    SlideInLeft,
    SlideInRight,
    SlideInUp,
    SlideInDown,
    ScaleIn,
    ScaleOut,
    RotateIn,
    Bounce
}
```

## 最佳实践

### 1. 组件组合

```valkyrie
# 复合组件示例
widget UserCard {
    user: User,
    show_actions: bool,
    
    new(user: User, show_actions: bool = true) {
        self.user = user
        self.show_actions = show_actions
    }
    
    render(self) -> Element {
        let mut card = Container::new()
            .padding(Padding::all(16.0))
            .border(Border::all(1.0, Color::Gray))
            .border_radius(8.0)
            .background(Color::White)
        
        # 用户头像和信息
        let user_info = FlexBox::new(FlexDirection::Row)
            .gap(12.0)
            .add_flex_child(
                Box::new(Avatar::new(self.user.avatar_url)
                    .size(48.0)),
                0.0, 1.0, FlexBasis::Auto
            )
            .add_flex_child(
                Box::new(FlexBox::new(FlexDirection::Column)
                    .add_flex_child(
                        Box::new(Text::new(self.user.name)
                            .size(TextSize::Large)
                            .weight(FontWeight::Bold)),
                        0.0, 1.0, FlexBasis::Auto
                    )
                    .add_flex_child(
                        Box::new(Text::new(self.user.email)
                            .color(Color::Gray)),
                        0.0, 1.0, FlexBasis::Auto
                    )),
                1.0, 1.0, FlexBasis::Auto
            )
        
        card = card.add_child(Box::new(user_info))
        
        # 操作按钮
        if self.show_actions {
            let actions = FlexBox::new(FlexDirection::Row)
                .gap(8.0)
                .justify_content(JustifyContent::End)
                .add_flex_child(
                    Box::new(Button::new("编辑")
                        .variant(ButtonVariant::Outline)),
                    0.0, 1.0, FlexBasis::Auto
                )
                .add_flex_child(
                    Box::new(Button::new("删除")
                        .variant(ButtonVariant::Danger)),
                    0.0, 1.0, FlexBasis::Auto
                )
            
            card = card.add_child(Box::new(actions))
        }
        
        card.render()
    }
}
```

### 2. 状态管理

```valkyrie
# 使用状态管理的组件
widget TodoApp {
    todos: [Todo],
    filter: TodoFilter,
    new_todo_text: String,
    
    new() {
        self.todos = []
        self.filter = TodoFilter::All
        self.new_todo_text = ""
    }
    
    render(self) -> Element {
        Container::new()
            .padding(Padding::all(20.0))
            .add_child(Box::new(self.render_header()))
            .add_child(Box::new(self.render_todo_list()))
            .add_child(Box::new(self.render_footer()))
            .render()
    }
    
    render_header(self) -> impl Widget {
        FlexBox::new(FlexDirection::Row)
            .gap(10.0)
            .add_flex_child(
                Box::new(TextInput::new("添加新任务...")
                    .value(self.new_todo_text)
                    .on_change { self.new_todo_text = $text }
                    .on_enter { self.add_todo() }),
                1.0, 1.0, FlexBasis::Auto
            )
            .add_flex_child(
                Box::new(Button::new("添加")
                    .on_click { self.add_todo() }),
                0.0, 1.0, FlexBasis::Auto
            )
    }
    
    add_todo(mut self) {
        if !self.new_todo_text.is_empty() {
            self.todos.push(Todo {
                id: generate_id(),
                text: self.new_todo_text.clone(),
                completed: false,
            })
            self.new_todo_text = ""
            self.request_update()
        }
    }
}
```

Widget 类型为 Valkyrie 提供了强大的 UI 开发能力，通过声明式的组件模型和响应式的状态管理，使得构建现代用户界面变得简单而高效。