package com.screenbuddy.android.data.local;

import androidx.annotation.NonNull;
import androidx.room.DatabaseConfiguration;
import androidx.room.InvalidationTracker;
import androidx.room.RoomDatabase;
import androidx.room.RoomOpenHelper;
import androidx.room.migration.AutoMigrationSpec;
import androidx.room.migration.Migration;
import androidx.room.util.DBUtil;
import androidx.room.util.TableInfo;
import androidx.sqlite.db.SupportSQLiteDatabase;
import androidx.sqlite.db.SupportSQLiteOpenHelper;
import java.lang.Class;
import java.lang.Override;
import java.lang.String;
import java.lang.SuppressWarnings;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import javax.annotation.processing.Generated;

@Generated("androidx.room.RoomProcessor")
@SuppressWarnings({"unchecked", "deprecation"})
public final class AppDatabase_Impl extends AppDatabase {
  private volatile ApiKeyDao _apiKeyDao;

  private volatile ModelDao _modelDao;

  @Override
  @NonNull
  protected SupportSQLiteOpenHelper createOpenHelper(@NonNull final DatabaseConfiguration config) {
    final SupportSQLiteOpenHelper.Callback _openCallback = new RoomOpenHelper(config, new RoomOpenHelper.Delegate(1) {
      @Override
      public void createAllTables(@NonNull final SupportSQLiteDatabase db) {
        db.execSQL("CREATE TABLE IF NOT EXISTS `api_keys` (`providerId` TEXT NOT NULL, `keyValue` TEXT NOT NULL, `isSet` INTEGER NOT NULL, `lastValidated` INTEGER, `lastUsed` INTEGER, PRIMARY KEY(`providerId`))");
        db.execSQL("CREATE TABLE IF NOT EXISTS `models` (`id` TEXT NOT NULL, `providerId` TEXT NOT NULL, `name` TEXT NOT NULL, `displayName` TEXT NOT NULL, `isFree` INTEGER NOT NULL, `isEnabled` INTEGER NOT NULL, `description` TEXT NOT NULL, `maxTokens` INTEGER NOT NULL, `costPer1k` REAL NOT NULL, PRIMARY KEY(`id`))");
        db.execSQL("CREATE TABLE IF NOT EXISTS room_master_table (id INTEGER PRIMARY KEY,identity_hash TEXT)");
        db.execSQL("INSERT OR REPLACE INTO room_master_table (id,identity_hash) VALUES(42, '44bc2d46666b72492ddff8d926d44c5e')");
      }

      @Override
      public void dropAllTables(@NonNull final SupportSQLiteDatabase db) {
        db.execSQL("DROP TABLE IF EXISTS `api_keys`");
        db.execSQL("DROP TABLE IF EXISTS `models`");
        final List<? extends RoomDatabase.Callback> _callbacks = mCallbacks;
        if (_callbacks != null) {
          for (RoomDatabase.Callback _callback : _callbacks) {
            _callback.onDestructiveMigration(db);
          }
        }
      }

      @Override
      public void onCreate(@NonNull final SupportSQLiteDatabase db) {
        final List<? extends RoomDatabase.Callback> _callbacks = mCallbacks;
        if (_callbacks != null) {
          for (RoomDatabase.Callback _callback : _callbacks) {
            _callback.onCreate(db);
          }
        }
      }

      @Override
      public void onOpen(@NonNull final SupportSQLiteDatabase db) {
        mDatabase = db;
        internalInitInvalidationTracker(db);
        final List<? extends RoomDatabase.Callback> _callbacks = mCallbacks;
        if (_callbacks != null) {
          for (RoomDatabase.Callback _callback : _callbacks) {
            _callback.onOpen(db);
          }
        }
      }

      @Override
      public void onPreMigrate(@NonNull final SupportSQLiteDatabase db) {
        DBUtil.dropFtsSyncTriggers(db);
      }

      @Override
      public void onPostMigrate(@NonNull final SupportSQLiteDatabase db) {
      }

      @Override
      @NonNull
      public RoomOpenHelper.ValidationResult onValidateSchema(
          @NonNull final SupportSQLiteDatabase db) {
        final HashMap<String, TableInfo.Column> _columnsApiKeys = new HashMap<String, TableInfo.Column>(5);
        _columnsApiKeys.put("providerId", new TableInfo.Column("providerId", "TEXT", true, 1, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsApiKeys.put("keyValue", new TableInfo.Column("keyValue", "TEXT", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsApiKeys.put("isSet", new TableInfo.Column("isSet", "INTEGER", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsApiKeys.put("lastValidated", new TableInfo.Column("lastValidated", "INTEGER", false, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsApiKeys.put("lastUsed", new TableInfo.Column("lastUsed", "INTEGER", false, 0, null, TableInfo.CREATED_FROM_ENTITY));
        final HashSet<TableInfo.ForeignKey> _foreignKeysApiKeys = new HashSet<TableInfo.ForeignKey>(0);
        final HashSet<TableInfo.Index> _indicesApiKeys = new HashSet<TableInfo.Index>(0);
        final TableInfo _infoApiKeys = new TableInfo("api_keys", _columnsApiKeys, _foreignKeysApiKeys, _indicesApiKeys);
        final TableInfo _existingApiKeys = TableInfo.read(db, "api_keys");
        if (!_infoApiKeys.equals(_existingApiKeys)) {
          return new RoomOpenHelper.ValidationResult(false, "api_keys(com.screenbuddy.android.data.local.ApiKeyEntity).\n"
                  + " Expected:\n" + _infoApiKeys + "\n"
                  + " Found:\n" + _existingApiKeys);
        }
        final HashMap<String, TableInfo.Column> _columnsModels = new HashMap<String, TableInfo.Column>(9);
        _columnsModels.put("id", new TableInfo.Column("id", "TEXT", true, 1, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("providerId", new TableInfo.Column("providerId", "TEXT", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("name", new TableInfo.Column("name", "TEXT", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("displayName", new TableInfo.Column("displayName", "TEXT", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("isFree", new TableInfo.Column("isFree", "INTEGER", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("isEnabled", new TableInfo.Column("isEnabled", "INTEGER", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("description", new TableInfo.Column("description", "TEXT", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("maxTokens", new TableInfo.Column("maxTokens", "INTEGER", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        _columnsModels.put("costPer1k", new TableInfo.Column("costPer1k", "REAL", true, 0, null, TableInfo.CREATED_FROM_ENTITY));
        final HashSet<TableInfo.ForeignKey> _foreignKeysModels = new HashSet<TableInfo.ForeignKey>(0);
        final HashSet<TableInfo.Index> _indicesModels = new HashSet<TableInfo.Index>(0);
        final TableInfo _infoModels = new TableInfo("models", _columnsModels, _foreignKeysModels, _indicesModels);
        final TableInfo _existingModels = TableInfo.read(db, "models");
        if (!_infoModels.equals(_existingModels)) {
          return new RoomOpenHelper.ValidationResult(false, "models(com.screenbuddy.android.data.local.ModelEntity).\n"
                  + " Expected:\n" + _infoModels + "\n"
                  + " Found:\n" + _existingModels);
        }
        return new RoomOpenHelper.ValidationResult(true, null);
      }
    }, "44bc2d46666b72492ddff8d926d44c5e", "be3ad7832c4cb5fede8efa3dcd8d26ab");
    final SupportSQLiteOpenHelper.Configuration _sqliteConfig = SupportSQLiteOpenHelper.Configuration.builder(config.context).name(config.name).callback(_openCallback).build();
    final SupportSQLiteOpenHelper _helper = config.sqliteOpenHelperFactory.create(_sqliteConfig);
    return _helper;
  }

  @Override
  @NonNull
  protected InvalidationTracker createInvalidationTracker() {
    final HashMap<String, String> _shadowTablesMap = new HashMap<String, String>(0);
    final HashMap<String, Set<String>> _viewTables = new HashMap<String, Set<String>>(0);
    return new InvalidationTracker(this, _shadowTablesMap, _viewTables, "api_keys","models");
  }

  @Override
  public void clearAllTables() {
    super.assertNotMainThread();
    final SupportSQLiteDatabase _db = super.getOpenHelper().getWritableDatabase();
    try {
      super.beginTransaction();
      _db.execSQL("DELETE FROM `api_keys`");
      _db.execSQL("DELETE FROM `models`");
      super.setTransactionSuccessful();
    } finally {
      super.endTransaction();
      _db.query("PRAGMA wal_checkpoint(FULL)").close();
      if (!_db.inTransaction()) {
        _db.execSQL("VACUUM");
      }
    }
  }

  @Override
  @NonNull
  protected Map<Class<?>, List<Class<?>>> getRequiredTypeConverters() {
    final HashMap<Class<?>, List<Class<?>>> _typeConvertersMap = new HashMap<Class<?>, List<Class<?>>>();
    _typeConvertersMap.put(ApiKeyDao.class, ApiKeyDao_Impl.getRequiredConverters());
    _typeConvertersMap.put(ModelDao.class, ModelDao_Impl.getRequiredConverters());
    return _typeConvertersMap;
  }

  @Override
  @NonNull
  public Set<Class<? extends AutoMigrationSpec>> getRequiredAutoMigrationSpecs() {
    final HashSet<Class<? extends AutoMigrationSpec>> _autoMigrationSpecsSet = new HashSet<Class<? extends AutoMigrationSpec>>();
    return _autoMigrationSpecsSet;
  }

  @Override
  @NonNull
  public List<Migration> getAutoMigrations(
      @NonNull final Map<Class<? extends AutoMigrationSpec>, AutoMigrationSpec> autoMigrationSpecs) {
    final List<Migration> _autoMigrations = new ArrayList<Migration>();
    return _autoMigrations;
  }

  @Override
  public ApiKeyDao apiKeyDao() {
    if (_apiKeyDao != null) {
      return _apiKeyDao;
    } else {
      synchronized(this) {
        if(_apiKeyDao == null) {
          _apiKeyDao = new ApiKeyDao_Impl(this);
        }
        return _apiKeyDao;
      }
    }
  }

  @Override
  public ModelDao modelDao() {
    if (_modelDao != null) {
      return _modelDao;
    } else {
      synchronized(this) {
        if(_modelDao == null) {
          _modelDao = new ModelDao_Impl(this);
        }
        return _modelDao;
      }
    }
  }
}
