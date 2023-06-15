pub struct Trader {
    policy: Vec<f64>, // 行動の確率を格納する。これは各トレーダーのポリシーを表します。
    reward: f64,      // 最新の報酬。これは最新の行動の結果として更新されます。
}

impl Trader {
    pub fn new() -> Trader {
        Trader {
            policy: vec![0.33, 0.33, 0.34], // 初期ポリシー。各行動を等確率で選ぶ
            reward: 0.0,
        }
    }

    // これは単純なランダムポリシーです。報酬に基づいてポリシーを更新するロジックは追加する必要があります。
    pub fn select_action(&self) -> usize {
        let random_value: f64 = rand::random();
        if random_value < self.policy[0] {
            return 0;
        } else if random_value < self.policy[0] + self.policy[1] {
            return 1;
        } else {
            return 2;
        }
    }

    // 報酬に基づいてポリシーを更新するロジックを追加する必要があります。
    pub fn update_policy(&mut self, reward: f64) {
        self.reward = reward;
        // ポリシー更新ロジックをここに書く
    }
}