pub fn main() -> Result<(), jonnah_slicer::Error> {
    {
        let bms_rs::bms::BmsOutput { bms, warnings } = bms_rs::bms::parse_bms(
            &std::fs::read_to_string(std::env::args().nth(1).ok_or("no args")?)?,
            bms_rs::bms::default_config(),
        );

        let bms = bms?;

        dbg!(warnings);

        let dbg = format!("{bms:#?}");
        std::fs::write("bad.json", &dbg)?;
    }

    let project = jonnah_slicer::project::load_project("projects/new_project")?;

    let bms = bms_rs::bms::model::Bms::try_from(project)?;

    let s = bms
        .unparse::<bms_rs::bms::command::channel::mapper::KeyLayoutBeat>()
        .into_iter()
        .map(|token| token.to_string())
        .collect::<Vec<String>>()
        .join("\n");

    std::fs::write("new_bms.bms", s)?;

    Ok(())
}
