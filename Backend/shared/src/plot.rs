use plotters::prelude::*;

pub fn plot(
    project_id: &str,
    script_id: &str,
    test_id: &str,
    target_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let root_area = BitMapBackend::new(target_file, (1024, 768)).into_drawing_area();

    root_area.fill(&WHITE)?;

    //let root_area = root_area.titled("Image Title", ("sans-serif", 60))?;

    let res = crate::get_parsed_results_history(project_id, script_id, test_id).ok_or("Plot Error")?;
    let start_datetime = res.iter().next().ok_or("Plot Error")?.datetime;
    let end_datetime = res.iter().last().ok_or("Plot Error")?.datetime;
    let max_max_response_time = res.iter().last().ok_or("Plot Error")?.total_max_response_time;
    let x_range =
        (start_datetime..end_datetime).with_key_points(vec![start_datetime, end_datetime]);
    let y_range = 0.0..max_max_response_time;

    let mut cc = ChartBuilder::on(&root_area)
        .margin((10).percent())
        .set_label_area_size(LabelAreaPosition::Left, (8).percent())
        .set_label_area_size(LabelAreaPosition::Bottom, (8).percent())
        .caption("Response Times", ("sans-serif", 40))
        .build_cartesian_2d(x_range, y_range)?; //this is the problem

    cc.configure_mesh()
        .x_labels(20)
        .y_labels(10)
        .disable_mesh()
        .x_label_formatter(&|v| format!("{:.1}", v))
        .y_label_formatter(&|v| format!("{:.1}", v))
        .x_desc("Date")
        .y_desc("Time (ms)")
        .draw()?;

    let total_average_response_time = LineSeries::new(
        res.iter()
            .map(|x| (x.datetime, x.total_average_response_time)),
        &RED,
    );
    cc.draw_series(total_average_response_time)?
        .label("Total Average Response Time")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));

    let total_median_response_time = LineSeries::new(
        res.iter()
            .map(|x| (x.datetime, x.total_median_response_time)),
        &BLUE,
    );
    cc.draw_series(total_median_response_time)?
        .label("Total Median Response Time")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &BLUE));

    let total_max_response_time = LineSeries::new(
        res.iter().map(|x| (x.datetime, x.total_max_response_time)),
        &GREEN,
    );
    cc.draw_series(total_max_response_time)?
        .label("Total Max Response Time")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &GREEN));

    let total_min_response_time = LineSeries::new(
        res.iter().map(|x| (x.datetime, x.total_min_response_time)),
        &YELLOW,
    );
    cc.draw_series(total_min_response_time)?
        .label("Total Min Response Time")
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &YELLOW));

    cc.configure_series_labels().border_style(&BLACK).draw()?;

    root_area.present()?;

    Ok(())
}
