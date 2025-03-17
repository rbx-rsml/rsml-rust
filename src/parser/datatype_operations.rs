// Words can not describe the pain this file fills me with. Eventually
// I will need to find a better solution for adding different datatypes 
// together. But this works fine for now i guess.

// TODO: Allow for tuples to have a number datatype
// specified to reduce the amount of casting.

use crate::parser::{Datatype, Operator};
use rbx_types::{CFrame, Color3, Matrix3, Rect, UDim, UDim2, Variant, Vector2, Vector2int16, Vector3, Vector3int16};

type OperationFn<N> = fn(N, N) -> N;

fn operation_number_with_number(
    left: f32, right: f32,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Float32(
        operation_fn_f32(left, right)
    ))
}

fn operation_number_with_udim(
    left: f32, right: UDim, operator: &Operator,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    if matches!(operator, Operator::Sub) {
        return Datatype::Variant(Variant::UDim(
            UDim::new(left + right.scale, -right.offset)
        ))
    };

    Datatype::Variant(Variant::UDim(
        UDim::new(operation_fn_f32(left, right.scale), right.offset)
    ))
}

fn operation_number_with_udim2(
    left: f32, right: UDim2, operator: &Operator,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    let (right_x, right_y) = (right.x, right.y);

    let solved_right_x = {
        if let Datatype::Variant(Variant::UDim(solved_right_x)) = 
            operation_number_with_udim(left, right_x, operator, operation_fn_f32) { solved_right_x } 
        else { unreachable!() }
    };

    let solved_right_y = {
        if let Datatype::Variant(Variant::UDim(solved_right_y)) = 
            operation_number_with_udim(left, right_y, operator, operation_fn_f32) { solved_right_y } 
        else { unreachable!() }
    };

    Datatype::Variant(Variant::UDim2(
        UDim2::new(solved_right_x, solved_right_y)
    ))
}

fn operation_number_with_vector3(
    left: f32, right: Vector3,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3(
        Vector3::new(
            operation_fn_f32(left, right.x),
            operation_fn_f32(left, right.y),
            operation_fn_f32(left, right.z),
        )
    ))
}

fn operation_number_with_vector3int16(
    left: f32, right: Vector3int16,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3int16(
        Vector3int16::new(
            operation_fn_f32(left, right.x as f32) as i16,
            operation_fn_f32(left, right.y as f32) as i16,
            operation_fn_f32(left, right.z as f32) as i16,
        )
    ))
}

fn operation_number_with_vector2(
    left: f32, right: Vector2,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector2(
        Vector2::new(
            operation_fn_f32(left, right.x),
            operation_fn_f32(left, right.y)
        )
    ))
}

fn operation_number_with_vector2int16(
    left: f32, right: Vector2int16,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector2int16(
        Vector2int16::new(
            operation_fn_f32(left, right.x as f32) as i16,
            operation_fn_f32(left, right.y as f32) as i16
        )
    ))
}

fn operation_number_with_rect(
    left: f32, right: Rect,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    let (right_min, right_max) = (right.min, right.max);

    Datatype::Variant(Variant::Rect(
        Rect::new(
            Vector2::new(
                operation_fn_f32(left, right_min.x), operation_fn_f32(left, right_min.y)
            ),
            Vector2::new(
                operation_fn_f32(left, right_max.x), operation_fn_f32(left, right_max.y)
            )
        )
    ))
}

fn operation_number_with_color3(
    left: f32, right: Color3,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Color3(
        Color3::new(
            operation_fn_f32(left, right.r),
            operation_fn_f32(left, right.g),
            operation_fn_f32(left, right.b)
        )
    ))
}


fn operation_udim_with_number(
    left: UDim, right: f32, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::UDim(
        UDim::new(operation_fn_f32(left.scale, right), left.offset)
    ))
}

fn operation_udim_with_udim(
    left: UDim, right: UDim, 
    operation_fn_f32: &OperationFn<f32>,
    operation_fn_i32: &OperationFn<i32>
) -> Datatype {
    Datatype::Variant(Variant::UDim(
        UDim::new(
            operation_fn_f32(left.scale, right.scale), 
            operation_fn_i32(left.offset, right.offset)
        )
    ))
}


fn operation_udim2_with_number(
    left: UDim2, right: f32,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    let (left_x, left_y) = (left.x, left.y);

    Datatype::Variant(Variant::UDim2(
        UDim2::new(
            UDim::new(operation_fn_f32(left_x.scale, right), left_x.offset),
            UDim::new(operation_fn_f32(left_y.scale, right), left_y.offset)
        )
    ))
}

fn operation_udim2_with_udim2(
    left: UDim2, right: UDim2, 
    operation_fn_f32: &OperationFn<f32>,
    operation_fn_i32: &OperationFn<i32>
) -> Datatype {
    let (left_x, left_y) = (left.x, left.y);
    let (right_x, right_y) = (right.x, right.y);

    Datatype::Variant(Variant::UDim2(
        UDim2::new(
            UDim::new(
                operation_fn_f32(left_x.scale, right_x.scale), 
                operation_fn_i32(left_x.offset, right_x.offset)
            ),
            UDim::new(
                operation_fn_f32(left_y.scale, right_y.scale), 
                operation_fn_i32(left_y.offset, right_y.offset)
            )
        )
    ))
}


fn operation_vector3_with_number(
    left: Vector3, right: f32, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3(
        Vector3::new(
            operation_fn_f32(left.x, right),
            operation_fn_f32(left.y, right),
            operation_fn_f32(left.z, right),
        )
    ))
}

fn operation_vector3_with_vector3int16(
    left: Vector3, right: Vector3int16, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3(
        Vector3::new(
            operation_fn_f32(left.x, right.x as f32),
            operation_fn_f32(left.y, right.y as f32),
            operation_fn_f32(left.z, right.z as f32),
        )
    ))
}

fn _operation_vector3_with_vector3(
    left: Vector3, right: Vector3, 
    operation_fn_f32: &OperationFn<f32>
) -> Vector3 {
    Vector3::new(
        operation_fn_f32(left.x, right.x),
        operation_fn_f32(left.y, right.y),
        operation_fn_f32(left.z, right.z),
    )
}
fn operation_vector3_with_vector3(
    left: Vector3, right: Vector3, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3(_operation_vector3_with_vector3(left, right, operation_fn_f32)))
}

fn operation_vector3int16_with_number(
    left: Vector3int16, right: f32, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3int16(
        Vector3int16::new(
            operation_fn_f32(left.x as f32, right) as i16,
            operation_fn_f32(left.y as f32, right) as i16,
            operation_fn_f32(left.z as f32, right) as i16,
        )
    ))
}

fn operation_vector3int16_with_vector3(
    left: Vector3int16, right: Vector3, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3int16(
        Vector3int16::new(
            operation_fn_f32(left.x as f32, right.x) as i16,
            operation_fn_f32(left.y as f32, right.y) as i16,
            operation_fn_f32(left.z as f32, right.z) as i16,
        )
    ))
}

fn operation_vector3int16_with_vector3int16(
    left: Vector3int16, right: Vector3int16, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector3int16(
        Vector3int16::new(
            operation_fn_f32(left.x as f32, right.x as f32) as i16,
            operation_fn_f32(left.y as f32, right.y as f32) as i16,
            operation_fn_f32(left.z as f32, right.z as f32) as i16,
        )
    ))
}

fn operation_cframe_with_cframe(
    left: CFrame, right: CFrame, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    let left_orien = left.orientation;
    let right_orien = right.orientation;

    Datatype::Variant(Variant::CFrame(
        CFrame::new(
            _operation_vector3_with_vector3(left.position, right.position, operation_fn_f32),
            Matrix3::new(
                _operation_vector3_with_vector3(left_orien.x, right_orien.x, operation_fn_f32),
                _operation_vector3_with_vector3(left_orien.y, right_orien.y, operation_fn_f32),
                _operation_vector3_with_vector3(left_orien.z, right_orien.z, operation_fn_f32)
            )
        )
    ))

    
}


fn operation_vector2_with_number(
    left: Vector2, right: f32, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector2(
        Vector2::new(
            operation_fn_f32(left.x, right),
            operation_fn_f32(left.y, right)
        )
    ))
}

fn operation_vector2_with_vector2int16(
    left: Vector2, right: Vector2int16, 
    operation_fn_f32: &OperationFn<f32>,
) -> Datatype {
    Datatype::Variant(Variant::Vector2(
        Vector2::new(
            operation_fn_f32(left.x, right.x as f32),
            operation_fn_f32(left.y, right.y as f32)
        )
    ))
}

fn operation_vector2_with_vector2(
    left: Vector2, right: Vector2, 
    operation_fn_f32: &OperationFn<f32>,
) -> Datatype {
    Datatype::Variant(Variant::Vector2(
        Vector2::new(
            operation_fn_f32(left.x, right.x),
            operation_fn_f32(left.y, right.y)
        )
    ))
}

fn operation_vector2int16_with_number(
    left: Vector2int16, right: f32, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Vector2int16(
        Vector2int16::new(
            operation_fn_f32(left.x as f32, right) as i16,
            operation_fn_f32(left.y as f32, right) as i16
        )
    ))
}

fn operation_vector2int16_with_vector2(
    left: Vector2int16, right: Vector2, 
    operation_fn_f32: &OperationFn<f32>,
) -> Datatype {
    Datatype::Variant(Variant::Vector2int16(
        Vector2int16::new(
            operation_fn_f32(left.x as f32, right.x) as i16,
            operation_fn_f32(left.y as f32, right.y) as i16
        )
    ))
}

fn operation_vector2int16_with_vector2int16(
    left: Vector2int16, right: Vector2int16, 
    operation_fn_f32: &OperationFn<f32>,
) -> Datatype {
    Datatype::Variant(Variant::Vector2int16(
        Vector2int16::new(
            operation_fn_f32(left.x as f32, right.x as f32) as i16,
            operation_fn_f32(left.y as f32, right.y as f32) as i16
        )
    ))
}

fn operation_rect_with_number(
    left: Rect, right: f32, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    let (left_min, left_max) = (left.min, left.max);

    Datatype::Variant(Variant::Rect(
        Rect::new(
            Vector2::new(
                operation_fn_f32(left_min.x, right), operation_fn_f32(left_min.y, right)
            ), 
            Vector2::new(
                operation_fn_f32(left_max.x, right), operation_fn_f32(left_max.y, right)
            )
        )
    ))
}

fn operation_rect_with_rect(
    left: Rect, right: Rect, 
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    let (left_min, left_max) = (left.min, left.max);
    let (right_min, right_max) = (right.min, right.max);

    Datatype::Variant(Variant::Rect(
        Rect::new(
            Vector2::new(
                operation_fn_f32(left_min.x, right_min.x), operation_fn_f32(left_min.y, right_min.y)
            ), 
            Vector2::new(
                operation_fn_f32(left_max.x, right_max.x), operation_fn_f32(left_max.y, right_max.y)
            )
        )
    ))
}


fn operation_color3_with_number(
    left: Color3, right: f32,
    operation_fn_f32: &OperationFn<f32>,
) -> Datatype {
    Datatype::Variant(Variant::Color3(
        Color3::new(
            operation_fn_f32(left.r, right),
            operation_fn_f32(left.g, right),
            operation_fn_f32(left.b, right)
        )
    ))
}


fn operation_color3_with_color3(
    left: Color3, right: Color3,
    operation_fn_f32: &OperationFn<f32>
) -> Datatype {
    Datatype::Variant(Variant::Color3(
        Color3::new(
            operation_fn_f32(left.r, right.r),
            operation_fn_f32(left.g, right.g),
            operation_fn_f32(left.b, right.b)
        )
    ))
}


pub fn datatype_operation(
    left: &Datatype, right: &Datatype, operator: &Operator,
    operation_fn_f32: &OperationFn<f32>, operation_fn_i32: &OperationFn<i32>
) -> Option<Datatype> {
    if let Datatype::Variant(Variant::Float32(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_number_with_number(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::UDim(right)) = right {
            return Some(operation_number_with_udim(*left, *right, operator, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::UDim2(right)) = right {
            return Some(operation_number_with_udim2(*left, *right, operator, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector3(right)) = right {
            return Some(operation_number_with_vector3(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector3int16(right)) = right {
            return Some(operation_number_with_vector3int16(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector2(right)) = right {
            return Some(operation_number_with_vector2(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector2int16(right)) = right {
            return Some(operation_number_with_vector2int16(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Rect(right)) = right {
            return Some(operation_number_with_rect(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Color3(right)) = right {
            return Some(operation_number_with_color3(*left, *right, operation_fn_f32))
        }
    }

    else if let Datatype::Variant(Variant::UDim(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_udim_with_number(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::UDim(right)) = right {
            return Some(operation_udim_with_udim(*left, *right, operation_fn_f32, operation_fn_i32))
        }
    }

    else if let Datatype::Variant(Variant::UDim2(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_udim2_with_number(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::UDim2(right)) = right {
            return Some(operation_udim2_with_udim2(*left, *right, operation_fn_f32, operation_fn_i32))
        }
    }

    else if let Datatype::Variant(Variant::Vector3(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_vector3_with_number(*left, *right, operation_fn_f32))
        }

        if let Datatype::Variant(Variant::Vector3int16(right)) = right {
            return Some(operation_vector3_with_vector3int16(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector3(right)) = right {
            return Some(operation_vector3_with_vector3(*left, *right, operation_fn_f32))
        }
    }

    else if let Datatype::Variant(Variant::Vector3int16(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_vector3int16_with_number(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector3(right)) = right {
            return Some(operation_vector3int16_with_vector3(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector3int16(right)) = right {
            return Some(operation_vector3int16_with_vector3int16(*left, *right, operation_fn_f32))
        }
    }

    else if let Datatype::Variant(Variant::CFrame(left)) = left {
        if let Datatype::Variant(Variant::CFrame(right)) = right {
            return Some(operation_cframe_with_cframe(*left, *right, operation_fn_f32))
        }
    }

    else if let Datatype::Variant(Variant::Vector2(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_vector2_with_number(*left, *right, operation_fn_f32))
        }

        if let Datatype::Variant(Variant::Vector2int16(right)) = right {
            return Some(operation_vector2_with_vector2int16(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector2(right)) = right {
            return Some(operation_vector2_with_vector2(*left, *right, operation_fn_f32))
        }
    }

    else if let Datatype::Variant(Variant::Vector2int16(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_vector2int16_with_number(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Vector2(right)) = right {
            return Some(operation_vector2int16_with_vector2(*left, *right, operation_fn_f32))
        }

        if let Datatype::Variant(Variant::Vector2int16(right)) = right {
            return Some(operation_vector2int16_with_vector2int16(*left, *right, operation_fn_f32))
        }
    }

    else if let Datatype::Variant(Variant::Rect(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_rect_with_number(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Rect(right)) = right {
            return Some(operation_rect_with_rect(*left, *right, operation_fn_f32))
        }
    }

    else if let Datatype::Variant(Variant::Color3(left)) = left {
        if let Datatype::Variant(Variant::Float32(right)) = right {
            return Some(operation_color3_with_number(*left, *right, operation_fn_f32))
        }

        else if let Datatype::Variant(Variant::Color3(right)) = right {
            return Some(operation_color3_with_color3(*left, *right, operation_fn_f32))
        }
    }

    None
}