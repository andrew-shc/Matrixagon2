use uom;


unit! {
    system: uom::si;
    quantity: uom::si::length;

    @blox: 1.0; "bx", "block", "blocks";
    @chux: 64.0; "cx", "chunk", "chunks";
}

// chux should be at 64.0 per blox

#[cfg(test)]
mod tests {
    use uom::fmt::DisplayStyle::Abbreviation;
    use uom::num_traits::Zero;
    use uom::si;
    use uom::si::f32::{Angle, Length, Ratio};
    use super::*;

    #[test]
    fn test_chunks_and_blocks() {
        let val = Length::new::<blox>(1.0);
        println!("{:?}", val);
        println!("{:?}", val.into_format_args(si::length::meter, Abbreviation));
        println!("{:?}", val.into_format_args(blox, Abbreviation));
        println!("{:?}", val.into_format_args(chux, Abbreviation));

        let val_cx = Length::new::<chux>(1.0);
        println!("{:?}", val_cx);
        println!("{:?}", val_cx.into_format_args(si::length::meter, Abbreviation));
        println!("{:?}", val_cx.into_format_args(blox, Abbreviation));
        println!("{:?}", val_cx.into_format_args(chux, Abbreviation));

        println!("--");

        let val = Length::new::<blox>(5.0);
        println!("{:?}", val);
        println!("{:?}", val.into_format_args(si::length::meter, Abbreviation));
        println!("{:?}", val.into_format_args(blox, Abbreviation));
        println!("{:?}", val.into_format_args(chux, Abbreviation));

        let val_cx = Length::new::<chux>(5.0);
        println!("{:?}", val_cx);
        println!("{:?}", val_cx.into_format_args(si::length::meter, Abbreviation));
        println!("{:?}", val_cx.into_format_args(blox, Abbreviation));
        println!("{:?}", val_cx.into_format_args(chux, Abbreviation));

        println!("--");

        let val = Length::new::<blox>(0.2);
        println!("{:?}", val);
        println!("{:?}", val.into_format_args(si::length::meter, Abbreviation));
        println!("{:?}", val.into_format_args(blox, Abbreviation));
        println!("{:?}", val.into_format_args(chux, Abbreviation));

        let val_cx = Length::new::<chux>(0.2);
        println!("{:?}", val_cx);
        println!("{:?}", val_cx.into_format_args(si::length::meter, Abbreviation));
        println!("{:?}", val_cx.into_format_args(blox, Abbreviation));
        println!("{:?}", val_cx.into_format_args(chux, Abbreviation));
    }

    #[test]
    fn test_angle_trig() {
        let ang = Angle::new::<si::angle::degree>(45.0);
        println!("{:?}", ang);
        println!("{:?}", ang.into_format_args(si::angle::degree, Abbreviation));
        println!("{:?}", ang.into_format_args(si::angle::radian, Abbreviation));

        println!("{:?}", ang.cos().into_format_args(si::ratio::ratio, Abbreviation));
        println!("{:?}", ang.tan().into_format_args(si::ratio::ratio, Abbreviation));

        println!("{:?}", ang.tan().value as f32);
    }

    struct TestStorage {
        x: si::f32::Length,
        y: si::f32::Length,
        z: si::f32::Length,
    }

    #[test]
    fn test_storage() {
        let test = TestStorage {
            x: si::f32::Length::new::<si::length::meter>(1.0),
            y: si::f32::Length::new::<blox>(2.0),
            z: si::f32::Length::new::<chux>(4.0),
        };

        println!("{:?}", test.x.into_format_args(blox, Abbreviation));
        println!("{:?}", test.y.into_format_args(blox, Abbreviation));
        println!("{:?}", test.z.into_format_args(blox, Abbreviation));
        println!("{:?}", test.z.get::<si::length::meter>());
        println!("{:?}", test.z.get::<chux>());
        println!("{:?}", test.y.get::<si::length::meter>());
        println!("{:?}", test.y.get::<chux>());
    }

    #[test]
    fn test_angle_equality() {
        let ang = Angle::new::<si::angle::degree>(0.0);

        println!("{:?}",ang.is_zero());
    }

    #[test]
    fn test_angle_prod() {
        let mut ang = Angle::new::<si::angle::degree>(4.0);
        let ang2 = Ratio::new::<si::ratio::ratio>(5.0);
        println!("{:?}", ang.is_zero());

        println!("{:?}", ang.into_format_args(si::angle::degree, Abbreviation));
        ang.value *= 2.0;
        println!("{:?}", ang.into_format_args(si::angle::degree, Abbreviation));
    }
}
