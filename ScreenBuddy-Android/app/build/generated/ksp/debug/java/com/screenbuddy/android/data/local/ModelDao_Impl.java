package com.screenbuddy.android.data.local;

import android.database.Cursor;
import android.os.CancellationSignal;
import androidx.annotation.NonNull;
import androidx.room.CoroutinesRoom;
import androidx.room.EntityDeletionOrUpdateAdapter;
import androidx.room.EntityInsertionAdapter;
import androidx.room.RoomDatabase;
import androidx.room.RoomSQLiteQuery;
import androidx.room.SharedSQLiteStatement;
import androidx.room.util.CursorUtil;
import androidx.room.util.DBUtil;
import androidx.sqlite.db.SupportSQLiteStatement;
import java.lang.Class;
import java.lang.Exception;
import java.lang.Integer;
import java.lang.Object;
import java.lang.Override;
import java.lang.String;
import java.lang.SuppressWarnings;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.Callable;
import javax.annotation.processing.Generated;
import kotlin.Unit;
import kotlin.coroutines.Continuation;
import kotlinx.coroutines.flow.Flow;

@Generated("androidx.room.RoomProcessor")
@SuppressWarnings({"unchecked", "deprecation"})
public final class ModelDao_Impl implements ModelDao {
  private final RoomDatabase __db;

  private final EntityInsertionAdapter<ModelEntity> __insertionAdapterOfModelEntity;

  private final EntityDeletionOrUpdateAdapter<ModelEntity> __updateAdapterOfModelEntity;

  private final SharedSQLiteStatement __preparedStmtOfSetAllModelsForProvider;

  private final SharedSQLiteStatement __preparedStmtOfSetAllPaidModels;

  private final SharedSQLiteStatement __preparedStmtOfSetAllModels;

  public ModelDao_Impl(@NonNull final RoomDatabase __db) {
    this.__db = __db;
    this.__insertionAdapterOfModelEntity = new EntityInsertionAdapter<ModelEntity>(__db) {
      @Override
      @NonNull
      protected String createQuery() {
        return "INSERT OR REPLACE INTO `models` (`id`,`providerId`,`name`,`displayName`,`isFree`,`isEnabled`,`description`,`maxTokens`,`costPer1k`) VALUES (?,?,?,?,?,?,?,?,?)";
      }

      @Override
      protected void bind(@NonNull final SupportSQLiteStatement statement,
          @NonNull final ModelEntity entity) {
        statement.bindString(1, entity.getId());
        statement.bindString(2, entity.getProviderId());
        statement.bindString(3, entity.getName());
        statement.bindString(4, entity.getDisplayName());
        final int _tmp = entity.isFree() ? 1 : 0;
        statement.bindLong(5, _tmp);
        final int _tmp_1 = entity.isEnabled() ? 1 : 0;
        statement.bindLong(6, _tmp_1);
        statement.bindString(7, entity.getDescription());
        statement.bindLong(8, entity.getMaxTokens());
        statement.bindDouble(9, entity.getCostPer1k());
      }
    };
    this.__updateAdapterOfModelEntity = new EntityDeletionOrUpdateAdapter<ModelEntity>(__db) {
      @Override
      @NonNull
      protected String createQuery() {
        return "UPDATE OR ABORT `models` SET `id` = ?,`providerId` = ?,`name` = ?,`displayName` = ?,`isFree` = ?,`isEnabled` = ?,`description` = ?,`maxTokens` = ?,`costPer1k` = ? WHERE `id` = ?";
      }

      @Override
      protected void bind(@NonNull final SupportSQLiteStatement statement,
          @NonNull final ModelEntity entity) {
        statement.bindString(1, entity.getId());
        statement.bindString(2, entity.getProviderId());
        statement.bindString(3, entity.getName());
        statement.bindString(4, entity.getDisplayName());
        final int _tmp = entity.isFree() ? 1 : 0;
        statement.bindLong(5, _tmp);
        final int _tmp_1 = entity.isEnabled() ? 1 : 0;
        statement.bindLong(6, _tmp_1);
        statement.bindString(7, entity.getDescription());
        statement.bindLong(8, entity.getMaxTokens());
        statement.bindDouble(9, entity.getCostPer1k());
        statement.bindString(10, entity.getId());
      }
    };
    this.__preparedStmtOfSetAllModelsForProvider = new SharedSQLiteStatement(__db) {
      @Override
      @NonNull
      public String createQuery() {
        final String _query = "UPDATE models SET isEnabled = ? WHERE providerId = ?";
        return _query;
      }
    };
    this.__preparedStmtOfSetAllPaidModels = new SharedSQLiteStatement(__db) {
      @Override
      @NonNull
      public String createQuery() {
        final String _query = "UPDATE models SET isEnabled = ? WHERE isFree = 0";
        return _query;
      }
    };
    this.__preparedStmtOfSetAllModels = new SharedSQLiteStatement(__db) {
      @Override
      @NonNull
      public String createQuery() {
        final String _query = "UPDATE models SET isEnabled = ?";
        return _query;
      }
    };
  }

  @Override
  public Object insertAll(final List<ModelEntity> models,
      final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        __db.beginTransaction();
        try {
          __insertionAdapterOfModelEntity.insert(models);
          __db.setTransactionSuccessful();
          return Unit.INSTANCE;
        } finally {
          __db.endTransaction();
        }
      }
    }, $completion);
  }

  @Override
  public Object update(final ModelEntity model, final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        __db.beginTransaction();
        try {
          __updateAdapterOfModelEntity.handle(model);
          __db.setTransactionSuccessful();
          return Unit.INSTANCE;
        } finally {
          __db.endTransaction();
        }
      }
    }, $completion);
  }

  @Override
  public Object setAllModelsForProvider(final String providerId, final boolean enabled,
      final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        final SupportSQLiteStatement _stmt = __preparedStmtOfSetAllModelsForProvider.acquire();
        int _argIndex = 1;
        final int _tmp = enabled ? 1 : 0;
        _stmt.bindLong(_argIndex, _tmp);
        _argIndex = 2;
        _stmt.bindString(_argIndex, providerId);
        try {
          __db.beginTransaction();
          try {
            _stmt.executeUpdateDelete();
            __db.setTransactionSuccessful();
            return Unit.INSTANCE;
          } finally {
            __db.endTransaction();
          }
        } finally {
          __preparedStmtOfSetAllModelsForProvider.release(_stmt);
        }
      }
    }, $completion);
  }

  @Override
  public Object setAllPaidModels(final boolean enabled,
      final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        final SupportSQLiteStatement _stmt = __preparedStmtOfSetAllPaidModels.acquire();
        int _argIndex = 1;
        final int _tmp = enabled ? 1 : 0;
        _stmt.bindLong(_argIndex, _tmp);
        try {
          __db.beginTransaction();
          try {
            _stmt.executeUpdateDelete();
            __db.setTransactionSuccessful();
            return Unit.INSTANCE;
          } finally {
            __db.endTransaction();
          }
        } finally {
          __preparedStmtOfSetAllPaidModels.release(_stmt);
        }
      }
    }, $completion);
  }

  @Override
  public Object setAllModels(final boolean enabled, final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        final SupportSQLiteStatement _stmt = __preparedStmtOfSetAllModels.acquire();
        int _argIndex = 1;
        final int _tmp = enabled ? 1 : 0;
        _stmt.bindLong(_argIndex, _tmp);
        try {
          __db.beginTransaction();
          try {
            _stmt.executeUpdateDelete();
            __db.setTransactionSuccessful();
            return Unit.INSTANCE;
          } finally {
            __db.endTransaction();
          }
        } finally {
          __preparedStmtOfSetAllModels.release(_stmt);
        }
      }
    }, $completion);
  }

  @Override
  public Flow<List<ModelEntity>> getAllModels() {
    final String _sql = "SELECT * FROM models";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 0);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"models"}, new Callable<List<ModelEntity>>() {
      @Override
      @NonNull
      public List<ModelEntity> call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfId = CursorUtil.getColumnIndexOrThrow(_cursor, "id");
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfName = CursorUtil.getColumnIndexOrThrow(_cursor, "name");
          final int _cursorIndexOfDisplayName = CursorUtil.getColumnIndexOrThrow(_cursor, "displayName");
          final int _cursorIndexOfIsFree = CursorUtil.getColumnIndexOrThrow(_cursor, "isFree");
          final int _cursorIndexOfIsEnabled = CursorUtil.getColumnIndexOrThrow(_cursor, "isEnabled");
          final int _cursorIndexOfDescription = CursorUtil.getColumnIndexOrThrow(_cursor, "description");
          final int _cursorIndexOfMaxTokens = CursorUtil.getColumnIndexOrThrow(_cursor, "maxTokens");
          final int _cursorIndexOfCostPer1k = CursorUtil.getColumnIndexOrThrow(_cursor, "costPer1k");
          final List<ModelEntity> _result = new ArrayList<ModelEntity>(_cursor.getCount());
          while (_cursor.moveToNext()) {
            final ModelEntity _item;
            final String _tmpId;
            _tmpId = _cursor.getString(_cursorIndexOfId);
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpName;
            _tmpName = _cursor.getString(_cursorIndexOfName);
            final String _tmpDisplayName;
            _tmpDisplayName = _cursor.getString(_cursorIndexOfDisplayName);
            final boolean _tmpIsFree;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsFree);
            _tmpIsFree = _tmp != 0;
            final boolean _tmpIsEnabled;
            final int _tmp_1;
            _tmp_1 = _cursor.getInt(_cursorIndexOfIsEnabled);
            _tmpIsEnabled = _tmp_1 != 0;
            final String _tmpDescription;
            _tmpDescription = _cursor.getString(_cursorIndexOfDescription);
            final int _tmpMaxTokens;
            _tmpMaxTokens = _cursor.getInt(_cursorIndexOfMaxTokens);
            final double _tmpCostPer1k;
            _tmpCostPer1k = _cursor.getDouble(_cursorIndexOfCostPer1k);
            _item = new ModelEntity(_tmpId,_tmpProviderId,_tmpName,_tmpDisplayName,_tmpIsFree,_tmpIsEnabled,_tmpDescription,_tmpMaxTokens,_tmpCostPer1k);
            _result.add(_item);
          }
          return _result;
        } finally {
          _cursor.close();
        }
      }

      @Override
      protected void finalize() {
        _statement.release();
      }
    });
  }

  @Override
  public Flow<List<ModelEntity>> getModelsByProvider(final String providerId) {
    final String _sql = "SELECT * FROM models WHERE providerId = ?";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 1);
    int _argIndex = 1;
    _statement.bindString(_argIndex, providerId);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"models"}, new Callable<List<ModelEntity>>() {
      @Override
      @NonNull
      public List<ModelEntity> call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfId = CursorUtil.getColumnIndexOrThrow(_cursor, "id");
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfName = CursorUtil.getColumnIndexOrThrow(_cursor, "name");
          final int _cursorIndexOfDisplayName = CursorUtil.getColumnIndexOrThrow(_cursor, "displayName");
          final int _cursorIndexOfIsFree = CursorUtil.getColumnIndexOrThrow(_cursor, "isFree");
          final int _cursorIndexOfIsEnabled = CursorUtil.getColumnIndexOrThrow(_cursor, "isEnabled");
          final int _cursorIndexOfDescription = CursorUtil.getColumnIndexOrThrow(_cursor, "description");
          final int _cursorIndexOfMaxTokens = CursorUtil.getColumnIndexOrThrow(_cursor, "maxTokens");
          final int _cursorIndexOfCostPer1k = CursorUtil.getColumnIndexOrThrow(_cursor, "costPer1k");
          final List<ModelEntity> _result = new ArrayList<ModelEntity>(_cursor.getCount());
          while (_cursor.moveToNext()) {
            final ModelEntity _item;
            final String _tmpId;
            _tmpId = _cursor.getString(_cursorIndexOfId);
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpName;
            _tmpName = _cursor.getString(_cursorIndexOfName);
            final String _tmpDisplayName;
            _tmpDisplayName = _cursor.getString(_cursorIndexOfDisplayName);
            final boolean _tmpIsFree;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsFree);
            _tmpIsFree = _tmp != 0;
            final boolean _tmpIsEnabled;
            final int _tmp_1;
            _tmp_1 = _cursor.getInt(_cursorIndexOfIsEnabled);
            _tmpIsEnabled = _tmp_1 != 0;
            final String _tmpDescription;
            _tmpDescription = _cursor.getString(_cursorIndexOfDescription);
            final int _tmpMaxTokens;
            _tmpMaxTokens = _cursor.getInt(_cursorIndexOfMaxTokens);
            final double _tmpCostPer1k;
            _tmpCostPer1k = _cursor.getDouble(_cursorIndexOfCostPer1k);
            _item = new ModelEntity(_tmpId,_tmpProviderId,_tmpName,_tmpDisplayName,_tmpIsFree,_tmpIsEnabled,_tmpDescription,_tmpMaxTokens,_tmpCostPer1k);
            _result.add(_item);
          }
          return _result;
        } finally {
          _cursor.close();
        }
      }

      @Override
      protected void finalize() {
        _statement.release();
      }
    });
  }

  @Override
  public Flow<List<ModelEntity>> getFreeModels() {
    final String _sql = "SELECT * FROM models WHERE isFree = 1";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 0);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"models"}, new Callable<List<ModelEntity>>() {
      @Override
      @NonNull
      public List<ModelEntity> call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfId = CursorUtil.getColumnIndexOrThrow(_cursor, "id");
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfName = CursorUtil.getColumnIndexOrThrow(_cursor, "name");
          final int _cursorIndexOfDisplayName = CursorUtil.getColumnIndexOrThrow(_cursor, "displayName");
          final int _cursorIndexOfIsFree = CursorUtil.getColumnIndexOrThrow(_cursor, "isFree");
          final int _cursorIndexOfIsEnabled = CursorUtil.getColumnIndexOrThrow(_cursor, "isEnabled");
          final int _cursorIndexOfDescription = CursorUtil.getColumnIndexOrThrow(_cursor, "description");
          final int _cursorIndexOfMaxTokens = CursorUtil.getColumnIndexOrThrow(_cursor, "maxTokens");
          final int _cursorIndexOfCostPer1k = CursorUtil.getColumnIndexOrThrow(_cursor, "costPer1k");
          final List<ModelEntity> _result = new ArrayList<ModelEntity>(_cursor.getCount());
          while (_cursor.moveToNext()) {
            final ModelEntity _item;
            final String _tmpId;
            _tmpId = _cursor.getString(_cursorIndexOfId);
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpName;
            _tmpName = _cursor.getString(_cursorIndexOfName);
            final String _tmpDisplayName;
            _tmpDisplayName = _cursor.getString(_cursorIndexOfDisplayName);
            final boolean _tmpIsFree;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsFree);
            _tmpIsFree = _tmp != 0;
            final boolean _tmpIsEnabled;
            final int _tmp_1;
            _tmp_1 = _cursor.getInt(_cursorIndexOfIsEnabled);
            _tmpIsEnabled = _tmp_1 != 0;
            final String _tmpDescription;
            _tmpDescription = _cursor.getString(_cursorIndexOfDescription);
            final int _tmpMaxTokens;
            _tmpMaxTokens = _cursor.getInt(_cursorIndexOfMaxTokens);
            final double _tmpCostPer1k;
            _tmpCostPer1k = _cursor.getDouble(_cursorIndexOfCostPer1k);
            _item = new ModelEntity(_tmpId,_tmpProviderId,_tmpName,_tmpDisplayName,_tmpIsFree,_tmpIsEnabled,_tmpDescription,_tmpMaxTokens,_tmpCostPer1k);
            _result.add(_item);
          }
          return _result;
        } finally {
          _cursor.close();
        }
      }

      @Override
      protected void finalize() {
        _statement.release();
      }
    });
  }

  @Override
  public Flow<List<ModelEntity>> getEnabledModels() {
    final String _sql = "SELECT * FROM models WHERE isEnabled = 1";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 0);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"models"}, new Callable<List<ModelEntity>>() {
      @Override
      @NonNull
      public List<ModelEntity> call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfId = CursorUtil.getColumnIndexOrThrow(_cursor, "id");
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfName = CursorUtil.getColumnIndexOrThrow(_cursor, "name");
          final int _cursorIndexOfDisplayName = CursorUtil.getColumnIndexOrThrow(_cursor, "displayName");
          final int _cursorIndexOfIsFree = CursorUtil.getColumnIndexOrThrow(_cursor, "isFree");
          final int _cursorIndexOfIsEnabled = CursorUtil.getColumnIndexOrThrow(_cursor, "isEnabled");
          final int _cursorIndexOfDescription = CursorUtil.getColumnIndexOrThrow(_cursor, "description");
          final int _cursorIndexOfMaxTokens = CursorUtil.getColumnIndexOrThrow(_cursor, "maxTokens");
          final int _cursorIndexOfCostPer1k = CursorUtil.getColumnIndexOrThrow(_cursor, "costPer1k");
          final List<ModelEntity> _result = new ArrayList<ModelEntity>(_cursor.getCount());
          while (_cursor.moveToNext()) {
            final ModelEntity _item;
            final String _tmpId;
            _tmpId = _cursor.getString(_cursorIndexOfId);
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpName;
            _tmpName = _cursor.getString(_cursorIndexOfName);
            final String _tmpDisplayName;
            _tmpDisplayName = _cursor.getString(_cursorIndexOfDisplayName);
            final boolean _tmpIsFree;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsFree);
            _tmpIsFree = _tmp != 0;
            final boolean _tmpIsEnabled;
            final int _tmp_1;
            _tmp_1 = _cursor.getInt(_cursorIndexOfIsEnabled);
            _tmpIsEnabled = _tmp_1 != 0;
            final String _tmpDescription;
            _tmpDescription = _cursor.getString(_cursorIndexOfDescription);
            final int _tmpMaxTokens;
            _tmpMaxTokens = _cursor.getInt(_cursorIndexOfMaxTokens);
            final double _tmpCostPer1k;
            _tmpCostPer1k = _cursor.getDouble(_cursorIndexOfCostPer1k);
            _item = new ModelEntity(_tmpId,_tmpProviderId,_tmpName,_tmpDisplayName,_tmpIsFree,_tmpIsEnabled,_tmpDescription,_tmpMaxTokens,_tmpCostPer1k);
            _result.add(_item);
          }
          return _result;
        } finally {
          _cursor.close();
        }
      }

      @Override
      protected void finalize() {
        _statement.release();
      }
    });
  }

  @Override
  public Flow<List<ModelEntity>> getEnabledModelsByProvider(final String providerId) {
    final String _sql = "SELECT * FROM models WHERE providerId = ? AND isEnabled = 1";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 1);
    int _argIndex = 1;
    _statement.bindString(_argIndex, providerId);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"models"}, new Callable<List<ModelEntity>>() {
      @Override
      @NonNull
      public List<ModelEntity> call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfId = CursorUtil.getColumnIndexOrThrow(_cursor, "id");
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfName = CursorUtil.getColumnIndexOrThrow(_cursor, "name");
          final int _cursorIndexOfDisplayName = CursorUtil.getColumnIndexOrThrow(_cursor, "displayName");
          final int _cursorIndexOfIsFree = CursorUtil.getColumnIndexOrThrow(_cursor, "isFree");
          final int _cursorIndexOfIsEnabled = CursorUtil.getColumnIndexOrThrow(_cursor, "isEnabled");
          final int _cursorIndexOfDescription = CursorUtil.getColumnIndexOrThrow(_cursor, "description");
          final int _cursorIndexOfMaxTokens = CursorUtil.getColumnIndexOrThrow(_cursor, "maxTokens");
          final int _cursorIndexOfCostPer1k = CursorUtil.getColumnIndexOrThrow(_cursor, "costPer1k");
          final List<ModelEntity> _result = new ArrayList<ModelEntity>(_cursor.getCount());
          while (_cursor.moveToNext()) {
            final ModelEntity _item;
            final String _tmpId;
            _tmpId = _cursor.getString(_cursorIndexOfId);
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpName;
            _tmpName = _cursor.getString(_cursorIndexOfName);
            final String _tmpDisplayName;
            _tmpDisplayName = _cursor.getString(_cursorIndexOfDisplayName);
            final boolean _tmpIsFree;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsFree);
            _tmpIsFree = _tmp != 0;
            final boolean _tmpIsEnabled;
            final int _tmp_1;
            _tmp_1 = _cursor.getInt(_cursorIndexOfIsEnabled);
            _tmpIsEnabled = _tmp_1 != 0;
            final String _tmpDescription;
            _tmpDescription = _cursor.getString(_cursorIndexOfDescription);
            final int _tmpMaxTokens;
            _tmpMaxTokens = _cursor.getInt(_cursorIndexOfMaxTokens);
            final double _tmpCostPer1k;
            _tmpCostPer1k = _cursor.getDouble(_cursorIndexOfCostPer1k);
            _item = new ModelEntity(_tmpId,_tmpProviderId,_tmpName,_tmpDisplayName,_tmpIsFree,_tmpIsEnabled,_tmpDescription,_tmpMaxTokens,_tmpCostPer1k);
            _result.add(_item);
          }
          return _result;
        } finally {
          _cursor.close();
        }
      }

      @Override
      protected void finalize() {
        _statement.release();
      }
    });
  }

  @Override
  public Object count(final Continuation<? super Integer> $completion) {
    final String _sql = "SELECT COUNT(*) FROM models";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 0);
    final CancellationSignal _cancellationSignal = DBUtil.createCancellationSignal();
    return CoroutinesRoom.execute(__db, false, _cancellationSignal, new Callable<Integer>() {
      @Override
      @NonNull
      public Integer call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final Integer _result;
          if (_cursor.moveToFirst()) {
            final int _tmp;
            _tmp = _cursor.getInt(0);
            _result = _tmp;
          } else {
            _result = 0;
          }
          return _result;
        } finally {
          _cursor.close();
          _statement.release();
        }
      }
    }, $completion);
  }

  @NonNull
  public static List<Class<?>> getRequiredConverters() {
    return Collections.emptyList();
  }
}
