table! {
    users (uid) {
        uid -> Text,
        play_key -> Text,
        display_name -> Text,
        connect_code -> Text,
        latest_version -> Nullable<Text>,
    }
}
