use debot_db::TransactionLog;
use rust_decimal::prelude::ToPrimitive;
use smartcore::linalg::basic::matrix::DenseMatrix;

pub async fn download_data(
    transaction_logs: &Vec<TransactionLog>,
    key: &str,
) -> (DenseMatrix<f64>, Vec<i32>, Vec<f64>) {
    let parts: Vec<&str> = key.split('_').collect();
    if parts.len() != 2 {
        panic!("Invalid key format. Expected format: <token_name>_<position_type>");
    }
    let token_name = parts[0];
    let position_type = parts[1];

    // Collect inputs and outputs from positions
    let mut inputs: Vec<Vec<f64>> = Vec::new();
    let mut output_classifier: Vec<i32> = Vec::new();
    let mut output_regressor: Vec<f64> = Vec::new();

    for transaction_log in transaction_logs {
        let db = transaction_log.get_r_db().await.expect("db is none");
        let positions = TransactionLog::get_all_open_positions(&db).await;

        for position in positions {
            if position.token_name == token_name
                && position.position_type == position_type
                && matches!(
                    position.state.as_str(),
                    "Closed(TakeProfit)" | "Closed(CutLoss)" | "Closed(Expired)"
                )
            {
                let debug_log = &position.debug;
                let mut input_vector = vec![
                    debug_log.input_1.to_f64().expect("conversion failed"),
                    debug_log.input_2.to_f64().expect("conversion failed"),
                    debug_log.input_3.to_f64().expect("conversion failed"),
                    debug_log.input_4.to_f64().expect("conversion failed"),
                    debug_log.input_5.to_f64().expect("conversion failed"),
                    debug_log.input_6.to_f64().expect("conversion failed"),
                    debug_log.input_7.to_f64().expect("conversion failed"),
                    debug_log.input_8.to_f64().expect("conversion failed"),
                    debug_log.input_9.to_f64().expect("conversion failed"),
                    debug_log.input_10.to_f64().expect("conversion failed"),
                    debug_log.input_11.to_f64().expect("conversion failed"),
                    debug_log.input_12.to_f64().expect("conversion failed"),
                    debug_log.input_13.to_f64().expect("conversion failed"),
                    debug_log.input_14.to_f64().expect("conversion failed"),
                    debug_log.input_15.to_f64().expect("conversion failed"),
                    debug_log.input_16.to_f64().expect("conversion failed"),
                    debug_log.input_17.to_f64().expect("conversion failed"),
                    debug_log.input_18.to_f64().expect("conversion failed"),
                    debug_log.input_19.to_f64().expect("conversion failed"),
                ];
                let candle_patterns = vec![
                    debug_log.input_20.to_one_hot(),
                    debug_log.input_21.to_one_hot(),
                    debug_log.input_22.to_one_hot(),
                    debug_log.input_23.to_one_hot(),
                ];
                for pattern in candle_patterns {
                    input_vector.extend(pattern.iter().map(|&d| d.to_f64().unwrap()));
                }
                inputs.push(input_vector);

                output_classifier.push(debug_log.output_1.to_i32().expect("conversion failed"));
                output_regressor.push(debug_log.output_2.to_f64().expect("conversion failed"));
            }
        }
        log::info!("num of positions = {}", inputs.len());
    }

    let count_class_0 = output_classifier.iter().filter(|&&x| x == 0).count();
    let count_class_1 = output_classifier.iter().filter(|&&x| x == 1).count();

    log::info!("num of inputs = {}", inputs.len());
    log::info!("Number of class 0 samples = {}", count_class_0);
    log::info!("Number of class 1 samples = {}", count_class_1);

    let input_slices: Vec<&[f64]> = inputs.iter().map(|v| v.as_slice()).collect();
    let x = DenseMatrix::from_2d_array(&input_slices);
    log::info!("dense matrix x = {:?}", x);

    (x, output_classifier, output_regressor)
}
