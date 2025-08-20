Review all the code for serial tests, make a scratch markdown listing all serial tests file and name.

Every serial tests needs to be no longer serial

If any test is serialized due to file access, use the IsolatedTestEnvironment guard so that tests are independent.

If any test are serialized due to in memory caches in the rust code, remove the caching -- there is no need for it.
