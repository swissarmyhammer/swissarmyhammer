
Get rid of these as yaml front matter -

- completed isn't 'in' the file -- it is being moved to completed
- file_path isn't 'in' the file -- it is from where it was loaded
- created_at isn't 'in' the file -- it is from the create date of the file. it also does not matter at all

We just DO NOT need yaml front matter in issues to make them work.

file path does not belong 'in' the yamls
    /// Whether the issue is completed
    pub completed: bool,
    /// The file path of the issue
    pub file_path: PathBuf,
    /// When the issue was created
    pub created_at: DateTime<Utc>,