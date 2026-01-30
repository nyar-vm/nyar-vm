# 单位制系统

Valkyrie 提供了强大的编译时单位制系统，通过宏和类型系统确保物理量计算的正确性，防止单位不匹配的错误。

## 基本单位定义

### SI基本单位

```valkyrie
# 基本单位宏
let mass = 1kg        # 千克
let length = 1m       # 米
let time = 1s         # 秒
let current = 1A      # 安培
let temperature = 1K  # 开尔文
let amount = 1mol     # 摩尔
let luminosity = 1cd  # 坎德拉

# 使用基本单位
let distance: Length = 100m
let duration: Time = 5s
let weight: Mass = 2.5kg
let temp: Temperature = 273.15K
```

### 导出单位

```valkyrie
# 面积单位
let area1 = 1m²       # 平方米
let area2 = 1m * 1m   # 等价写法

# 体积单位
let volume1 = 1m³     # 立方米
let volume2 = 1m * 1m * 1m  # 等价写法

# 速度单位
let velocity1 = 1m/s  # 米每秒
let velocity2 = 1m / 1s  # 等价写法

# 加速度单位
let acceleration = 1m/s²  # 米每秒平方

# 力单位
let force1 = 1N       # 牛顿
let force2 = 1kg * 1m/s²  # 等价定义

# 能量单位
let energy1 = 1J      # 焦耳
let energy2 = 1N * 1m # 等价定义
let energy3 = 1kg * 1m²/s²  # 基本单位表示

# 功率单位
let power1 = 1W       # 瓦特
let power2 = 1J/s     # 等价定义
```

## 单位类型系统

### 量纲类型

```valkyrie
# 量纲类型定义
type Length = Quantity⟨[1, 0, 0, 0, 0, 0, 0]⟩     # [L, M, T, I, Θ, N, J]
type Mass = Quantity⟨[0, 1, 0, 0, 0, 0, 0]⟩
type Time = Quantity⟨[0, 0, 1, 0, 0, 0, 0]⟩
type Area = Quantity⟨[2, 0, 0, 0, 0, 0, 0]⟩
type Volume = Quantity⟨[3, 0, 0, 0, 0, 0, 0]⟩
type Velocity = Quantity⟨[1, 0, -1, 0, 0, 0, 0]⟩
type Acceleration = Quantity⟨[1, 0, -2, 0, 0, 0, 0]⟩
type Force = Quantity⟨[1, 1, -2, 0, 0, 0, 0]⟩
type Energy = Quantity⟨[2, 1, -2, 0, 0, 0, 0]⟩
type Power = Quantity⟨[2, 1, -3, 0, 0, 0, 0]⟩

# 使用量纲类型
micro calculate_kinetic_energy(mass: Mass, velocity: Velocity) -> Energy {
    0.5 * mass * velocity * velocity
}

micro calculate_power(energy: Energy, time: Time) -> Power {
    energy / time
}
```

### 单位转换

```valkyrie
# 长度单位转换
let meter = 1m
let kilometer = 1km     # 1000m
let centimeter = 1cm    # 0.01m
let millimeter = 1mm    # 0.001m
let inch = 1inch        # 0.0254m
let foot = 1ft          # 0.3048m
let yard = 1yd          # 0.9144m
let mile = 1mile        # 1609.344m

# 自动转换
let distance1: Length = 5km
let distance2: Length = 3000m
let total_distance = distance1 + distance2  # 8000m

# 显式转换
let km_value = distance1.to(km)  # 5.0
let m_value = distance1.to(m)    # 5000.0
```

### 质量单位

```valkyrie
# 质量单位
let kilogram = 1kg
let gram = 1g           # 0.001kg
let ton = 1t            # 1000kg
let pound = 1lb         # 0.453592kg
let ounce = 1oz         # 0.0283495kg

# 质量计算
let total_mass = 2kg + 500g  # 2.5kg
let density = total_mass / (1m³)  # 密度类型
```

### 时间单位

```valkyrie
# 时间单位
let second = 1s
let minute = 1min       # 60s
let hour = 1h           # 3600s
let day = 1day          # 86400s
let week = 1week        # 604800s
let year = 1year        # 31557600s (365.25 days)

# 时间计算
let duration = 2h + 30min + 15s  # 9015s
let frequency = 1 / duration      # 频率类型
```

## 复合单位计算

### 物理公式

```valkyrie
# 牛顿第二定律: F = ma
micro newtons_second_law(mass: Mass, acceleration: Acceleration) -> Force {
    mass * acceleration
}

# 动能公式: E = 1/2 * m * v²
micro kinetic_energy(mass: Mass, velocity: Velocity) -> Energy {
    0.5 * mass * velocity.pow(2)
}

# 重力势能: E = mgh
micro gravitational_potential_energy(mass: Mass, height: Length, g: Acceleration) -> Energy {
    mass * g * height
}

# 功率公式: P = W/t
micro power_from_work(work: Energy, time: Time) -> Power {
    work / time
}

# 欧姆定律: V = IR
micro ohms_law(current: Current, resistance: Resistance) -> Voltage {
    current * resistance
}
```

### 电学单位

```valkyrie
# 电学基本单位
let current = 1A        # 安培
let voltage = 1V        # 伏特
let resistance = 1Ω     # 欧姆
let capacitance = 1F    # 法拉
let inductance = 1H     # 亨利
let charge = 1C         # 库仑

# 电学计算
let power_electrical = voltage * current  # 电功率
let energy_stored = 0.5 * capacitance * voltage.pow(2)  # 电容储能
let magnetic_energy = 0.5 * inductance * current.pow(2)  # 电感储能
```

### 热力学单位

```valkyrie
# 温度单位
let kelvin = 1K
let celsius = 1°C       # 相对温度
let fahrenheit = 1°F    # 相对温度

# 热量单位
let joule = 1J
let calorie = 1cal      # 4.184J
let btu = 1BTU          # 1055.06J

# 热力学计算
micro heat_capacity(heat: Energy, temp_change: Temperature) -> HeatCapacity {
    heat / temp_change
}

micro thermal_conductivity(heat_flow: Power, area: Area, temp_gradient: Temperature, length: Length) -> ThermalConductivity {
    heat_flow * length / (area * temp_gradient)
}
```

## 单位制验证

### 编译时检查

```valkyrie
# 正确的单位运算
let distance = 100m
let time = 10s
let speed = distance / time  # 10 m/s，类型正确

# 编译错误示例
# let invalid = distance + time  # 错误：不能将长度和时间相加
# let wrong_speed = distance * time  # 错误：结果不是速度类型

# 函数参数类型检查
micro calculate_work(force: Force, distance: Length) -> Energy {
    force * distance
}

# 调用时会进行类型检查
let work = calculate_work(10N, 5m)  # 正确
# let invalid_work = calculate_work(10m, 5N)  # 错误：参数类型不匹配
```

### 运行时单位转换

```valkyrie
# 单位转换函数
class UnitConverter {
    conversion_factors: HashMap<(Unit, Unit), f64>,
}

impl UnitConverter {
    micro convert⟨T: Dimension⟩(self, value: Quantity⟨T⟩, from: Unit, to: Unit) -> Quantity⟨T⟩ {
        if from == to {
            return value
        }
        
        let factor = self.conversion_factors.get(&(from, to))
            .or_else(|| self.conversion_factors.get(&(to, from)).map(|f| 1.0 / f))
            .expect("No conversion factor found")
        
        Quantity::new(value.value() * factor)
    }
}

# 使用转换器
let converter = UnitConverter::default()
let distance_m = converter.convert(5km, km, m)  # 5000m
let mass_kg = converter.convert(10lb, lb, kg)   # 4.53592kg
```

## 自定义单位系统

### 定义新的单位

```valkyrie
# 定义新的基本单位
macro_rules! define_unit {
    ($name:ident, $symbol:literal, $dimension:expr) => {
        pub class $name;
        
        impl Unit for $name {
            const SYMBOL: string = $symbol;
            type Dimension = $dimension;
        }
        
        pub const $name: Quantity⟨$dimension⟩ = Quantity::new(1.0);
    }
}

# 使用宏定义新单位
define_unit!(Pixel, "px", Length);  # 像素作为长度单位
define_unit!(Byte, "B", Information);  # 字节作为信息单位
define_unit!(Bit, "bit", Information);

# 信息单位计算
let file_size = 1024Byte
let bandwidth = file_size / 1s  # 字节每秒
```

### 复合单位宏

```valkyrie
# 定义复合单位的宏
macro_rules! compound_unit {
    ($name:ident = $($unit:ident)^$power:expr)*) => {
        type $name = Quantity<[
            $($unit::Dimension::EXPONENTS[0] * $power +)* 0,
            $($unit::Dimension::EXPONENTS[1] * $power +)* 0,
            $($unit::Dimension::EXPONENTS[2] * $power +)* 0,
            $($unit::Dimension::EXPONENTS[3] * $power +)* 0,
            $($unit::Dimension::EXPONENTS[4] * $power +)* 0,
            $($unit::Dimension::EXPONENTS[5] * $power +)* 0,
            $($unit::Dimension::EXPONENTS[6] * $power +)* 0,
        ]>;
    }
}

# 使用复合单位宏
compound_unit!(Density = Mass^1 Length^-3);  # kg/m³
compound_unit!(Pressure = Mass^1 Length^-1 Time^-2);  # Pa = kg/(m·s²)
compound_unit!(ElectricField = Mass^1 Length^1 Time^-3 Current^-1);  # V/m
```

## 单位制应用示例

### 物理仿真

```valkyrie
# 粒子物理仿真
class Particle {
    mass: Mass,
    position: Vector3⟨Length⟩,
    velocity: Vector3⟨Velocity⟩,
    acceleration: Vector3⟨Acceleration⟩,
}

impl Particle {
    micro apply_force(mut self, force: Vector3⟨Force⟩, dt: Time) {
        # F = ma => a = F/m
        self.acceleration = force / self.mass
        
        # 更新速度和位置
        self.velocity += self.acceleration * dt
        self.position += self.velocity * dt
    }
    
    micro kinetic_energy(self) -> Energy {
        0.5 * self.mass * self.velocity.magnitude_squared()
    }
}

# 重力场仿真
class GravityField {
    g: Acceleration,  # 重力加速度
}

impl GravityField {
    micro force_on(self, particle: Particle) -> Vector3⟨Force⟩ {
        Vector3::new(0, -particle.mass * self.g, 0)
    }
}
```

### 工程计算

```valkyrie
# 结构工程计算
class Beam {
    length: Length,
    cross_section_area: Area,
    moment_of_inertia: MomentOfInertia,
    elastic_modulus: Pressure,
}

impl Beam {
    micro max_deflection(self, load: Force) -> Length {
        # 简支梁中点最大挠度公式
        load * self.length.pow(3) / (48 * self.elastic_modulus * self.moment_of_inertia)
    }
    
    micro max_stress(self, moment: Moment, distance: Length) -> Pressure {
        moment * distance / self.moment_of_inertia
    }
}

# 流体力学计算
micro reynolds_number(density: Density, velocity: Velocity, length: Length, viscosity: DynamicViscosity) -> Dimensionless {
    density * velocity * length / viscosity
}

micro drag_force(drag_coefficient: Dimensionless, density: Density, velocity: Velocity, area: Area) -> Force {
    0.5 * drag_coefficient * density * velocity.pow(2) * area
}
```

### 电路分析

```valkyrie
# 电路元件
class Resistor {
    resistance: Resistance,
}

class Capacitor {
    capacitance: Capacitance,
}

class Inductor {
    inductance: Inductance,
}

# 电路分析函数
micro rc_time_constant(resistance: Resistance, capacitance: Capacitance) -> Time {
    resistance * capacitance
}

micro lc_resonant_frequency(inductance: Inductance, capacitance: Capacitance) -> Frequency {
    1 / (2 * PI * (inductance * capacitance).sqrt())
}

micro power_dissipation(voltage: Voltage, current: Current) -> Power {
    voltage * current
}
```

## 性能优化

### 零成本抽象

```valkyrie
# 编译时单位消除
#[inline(always)]
micro optimized_calculation(distance: Length, time: Time) -> Velocity {
    # 编译后等价于: distance_value / time_value
    distance / time
}

# 常量折叠
const GRAVITY: Acceleration = 9.81m/s²;
const EARTH_RADIUS: Length = 6.371e6m;

# 编译时计算
const ESCAPE_VELOCITY: Velocity = (2 * GRAVITY * EARTH_RADIUS).sqrt();
```

### 批量计算优化

```valkyrie
# SIMD向量化单位计算
class VectorQuantity⟨T: Dimension, const N: usize⟩ {
    values: [f64; N],
    _phantom: PhantomData⟨T⟩,
}

impl⟨T: Dimension, const N: usize⟩ VectorQuantity⟨T, N⟩ {
    micro add(self, other: Self) -> Self {
        let mut result = [0.0; N];
        for i in 0..N {
            result[i] = self.values[i] + other.values[i];
        }
        Self { values: result, _phantom: PhantomData }
    }
    
    micro multiply_scalar(self, scalar: f64) -> Self {
        let mut result = [0.0; N];
        for i in 0..N {
            result[i] = self.values[i] * scalar;
        }
        Self { values: result, _phantom: PhantomData }
    }
}
```

## 最佳实践

### 1. 单位一致性

```valkyrie
# 始终使用带单位的字面量
let good_distance = 100m;  # 好的做法
# let bad_distance = 100;  # 避免无单位数值

# 函数签名明确单位
micro calculate_area(width: Length, height: Length) -> Area {
    width * height
}

# 而不是
# micro calculate_area(width: f64, height: f64) -> f64
```

### 2. 单位转换策略

```valkyrie
# 在边界处进行单位转换
micro process_user_input(distance_str: string, unit_str: string) -> Result⟨Length, Any⟩ {
    let value: f64 = distance_str.parse()?;
    match unit_str {
        "m" => Ok(value * 1m),
        "km" => Ok(value * 1km),
        "ft" => Ok(value * 1ft),
        _ => Err(Any::UnknownUnit),
    }
}

# 内部计算保持一致单位
micro internal_calculation(distances: [Length]) -> Length {
    distances.iter().sum()  # 自动处理单位
}
```

### 3. 错误处理

```valkyrie
# 单位不匹配的错误处理
union UnitError {
    IncompatibleUnits { expected: string, found: string },
    ConversionNotFound { from: string, to: string },
    InvalidValue { value: f64 }
}

# 安全的单位操作
micro safe_divide⟨T: Dimension, U: Dimension⟩(numerator: Quantity⟨T⟩, denominator: Quantity⟨U⟩) -> Result⟨Quantity⟨T::Div⟨U⟩⟩, UnitError⟩ {
    if denominator.value().abs() < f64::EPSILON {
        return Err(UnitError::InvalidValue(denominator.value()))
    }
    Ok(numerator / denominator)
}
```

Valkyrie 的单位制系统通过编译时检查和零成本抽象，为科学计算和工程应用提供了类型安全的物理量计算能力，有效防止了单位错误导致的计算问题。