use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFolder {
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFolder {
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_folder_serialization() {
        let folder = Folder {
            id: 1,
            name: "测试文件夹".to_string(),
            parent_id: None,
            created_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        };

        let json = serde_json::to_string(&folder).unwrap();
        let deserialized: Folder = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.name, "测试文件夹");
        assert_eq!(deserialized.parent_id, None);
    }

    #[test]
    fn test_new_folder_serialization() {
        let new_folder = NewFolder {
            name: "新文件夹".to_string(),
            parent_id: Some(1),
        };

        let json = serde_json::to_string(&new_folder).unwrap();
        let deserialized: NewFolder = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "新文件夹");
        assert_eq!(deserialized.parent_id, Some(1));
    }

    #[test]
    fn test_folder_with_parent() {
        let folder = Folder {
            id: 2,
            name: "子文件夹".to_string(),
            parent_id: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(folder.parent_id.is_some());
        assert_eq!(folder.parent_id.unwrap(), 1);
    }
}