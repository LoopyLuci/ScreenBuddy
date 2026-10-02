package com.screenbuddy.android.data.local;

import android.database.Cursor;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
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
import java.lang.Long;
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
public final class ApiKeyDao_Impl implements ApiKeyDao {
  private final RoomDatabase __db;

  private final EntityInsertionAdapter<ApiKeyEntity> __insertionAdapterOfApiKeyEntity;

  private final EntityDeletionOrUpdateAdapter<ApiKeyEntity> __deletionAdapterOfApiKeyEntity;

  private final EntityDeletionOrUpdateAdapter<ApiKeyEntity> __updateAdapterOfApiKeyEntity;

  private final SharedSQLiteStatement __preparedStmtOfDeleteByProvider;

  private final SharedSQLiteStatement __preparedStmtOfClearAllKeys;

  public ApiKeyDao_Impl(@NonNull final RoomDatabase __db) {
    this.__db = __db;
    this.__insertionAdapterOfApiKeyEntity = new EntityInsertionAdapter<ApiKeyEntity>(__db) {
      @Override
      @NonNull
      protected String createQuery() {
        return "INSERT OR REPLACE INTO `api_keys` (`providerId`,`keyValue`,`isSet`,`lastValidated`,`lastUsed`) VALUES (?,?,?,?,?)";
      }

      @Override
      protected void bind(@NonNull final SupportSQLiteStatement statement,
          @NonNull final ApiKeyEntity entity) {
        statement.bindString(1, entity.getProviderId());
        statement.bindString(2, entity.getKeyValue());
        final int _tmp = entity.isSet() ? 1 : 0;
        statement.bindLong(3, _tmp);
        if (entity.getLastValidated() == null) {
          statement.bindNull(4);
        } else {
          statement.bindLong(4, entity.getLastValidated());
        }
        if (entity.getLastUsed() == null) {
          statement.bindNull(5);
        } else {
          statement.bindLong(5, entity.getLastUsed());
        }
      }
    };
    this.__deletionAdapterOfApiKeyEntity = new EntityDeletionOrUpdateAdapter<ApiKeyEntity>(__db) {
      @Override
      @NonNull
      protected String createQuery() {
        return "DELETE FROM `api_keys` WHERE `providerId` = ?";
      }

      @Override
      protected void bind(@NonNull final SupportSQLiteStatement statement,
          @NonNull final ApiKeyEntity entity) {
        statement.bindString(1, entity.getProviderId());
      }
    };
    this.__updateAdapterOfApiKeyEntity = new EntityDeletionOrUpdateAdapter<ApiKeyEntity>(__db) {
      @Override
      @NonNull
      protected String createQuery() {
        return "UPDATE OR ABORT `api_keys` SET `providerId` = ?,`keyValue` = ?,`isSet` = ?,`lastValidated` = ?,`lastUsed` = ? WHERE `providerId` = ?";
      }

      @Override
      protected void bind(@NonNull final SupportSQLiteStatement statement,
          @NonNull final ApiKeyEntity entity) {
        statement.bindString(1, entity.getProviderId());
        statement.bindString(2, entity.getKeyValue());
        final int _tmp = entity.isSet() ? 1 : 0;
        statement.bindLong(3, _tmp);
        if (entity.getLastValidated() == null) {
          statement.bindNull(4);
        } else {
          statement.bindLong(4, entity.getLastValidated());
        }
        if (entity.getLastUsed() == null) {
          statement.bindNull(5);
        } else {
          statement.bindLong(5, entity.getLastUsed());
        }
        statement.bindString(6, entity.getProviderId());
      }
    };
    this.__preparedStmtOfDeleteByProvider = new SharedSQLiteStatement(__db) {
      @Override
      @NonNull
      public String createQuery() {
        final String _query = "DELETE FROM api_keys WHERE providerId = ?";
        return _query;
      }
    };
    this.__preparedStmtOfClearAllKeys = new SharedSQLiteStatement(__db) {
      @Override
      @NonNull
      public String createQuery() {
        final String _query = "UPDATE api_keys SET isSet = 0";
        return _query;
      }
    };
  }

  @Override
  public Object insert(final ApiKeyEntity apiKey, final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        __db.beginTransaction();
        try {
          __insertionAdapterOfApiKeyEntity.insert(apiKey);
          __db.setTransactionSuccessful();
          return Unit.INSTANCE;
        } finally {
          __db.endTransaction();
        }
      }
    }, $completion);
  }

  @Override
  public Object insertAll(final List<ApiKeyEntity> apiKeys,
      final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        __db.beginTransaction();
        try {
          __insertionAdapterOfApiKeyEntity.insert(apiKeys);
          __db.setTransactionSuccessful();
          return Unit.INSTANCE;
        } finally {
          __db.endTransaction();
        }
      }
    }, $completion);
  }

  @Override
  public Object delete(final ApiKeyEntity apiKey, final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        __db.beginTransaction();
        try {
          __deletionAdapterOfApiKeyEntity.handle(apiKey);
          __db.setTransactionSuccessful();
          return Unit.INSTANCE;
        } finally {
          __db.endTransaction();
        }
      }
    }, $completion);
  }

  @Override
  public Object update(final ApiKeyEntity apiKey, final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        __db.beginTransaction();
        try {
          __updateAdapterOfApiKeyEntity.handle(apiKey);
          __db.setTransactionSuccessful();
          return Unit.INSTANCE;
        } finally {
          __db.endTransaction();
        }
      }
    }, $completion);
  }

  @Override
  public Object deleteByProvider(final String providerId,
      final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        final SupportSQLiteStatement _stmt = __preparedStmtOfDeleteByProvider.acquire();
        int _argIndex = 1;
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
          __preparedStmtOfDeleteByProvider.release(_stmt);
        }
      }
    }, $completion);
  }

  @Override
  public Object clearAllKeys(final Continuation<? super Unit> $completion) {
    return CoroutinesRoom.execute(__db, true, new Callable<Unit>() {
      @Override
      @NonNull
      public Unit call() throws Exception {
        final SupportSQLiteStatement _stmt = __preparedStmtOfClearAllKeys.acquire();
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
          __preparedStmtOfClearAllKeys.release(_stmt);
        }
      }
    }, $completion);
  }

  @Override
  public Flow<List<ApiKeyEntity>> getAllApiKeys() {
    final String _sql = "SELECT * FROM api_keys";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 0);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"api_keys"}, new Callable<List<ApiKeyEntity>>() {
      @Override
      @NonNull
      public List<ApiKeyEntity> call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfKeyValue = CursorUtil.getColumnIndexOrThrow(_cursor, "keyValue");
          final int _cursorIndexOfIsSet = CursorUtil.getColumnIndexOrThrow(_cursor, "isSet");
          final int _cursorIndexOfLastValidated = CursorUtil.getColumnIndexOrThrow(_cursor, "lastValidated");
          final int _cursorIndexOfLastUsed = CursorUtil.getColumnIndexOrThrow(_cursor, "lastUsed");
          final List<ApiKeyEntity> _result = new ArrayList<ApiKeyEntity>(_cursor.getCount());
          while (_cursor.moveToNext()) {
            final ApiKeyEntity _item;
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpKeyValue;
            _tmpKeyValue = _cursor.getString(_cursorIndexOfKeyValue);
            final boolean _tmpIsSet;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsSet);
            _tmpIsSet = _tmp != 0;
            final Long _tmpLastValidated;
            if (_cursor.isNull(_cursorIndexOfLastValidated)) {
              _tmpLastValidated = null;
            } else {
              _tmpLastValidated = _cursor.getLong(_cursorIndexOfLastValidated);
            }
            final Long _tmpLastUsed;
            if (_cursor.isNull(_cursorIndexOfLastUsed)) {
              _tmpLastUsed = null;
            } else {
              _tmpLastUsed = _cursor.getLong(_cursorIndexOfLastUsed);
            }
            _item = new ApiKeyEntity(_tmpProviderId,_tmpKeyValue,_tmpIsSet,_tmpLastValidated,_tmpLastUsed);
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
  public Flow<ApiKeyEntity> getApiKey(final String providerId) {
    final String _sql = "SELECT * FROM api_keys WHERE providerId = ?";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 1);
    int _argIndex = 1;
    _statement.bindString(_argIndex, providerId);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"api_keys"}, new Callable<ApiKeyEntity>() {
      @Override
      @Nullable
      public ApiKeyEntity call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfKeyValue = CursorUtil.getColumnIndexOrThrow(_cursor, "keyValue");
          final int _cursorIndexOfIsSet = CursorUtil.getColumnIndexOrThrow(_cursor, "isSet");
          final int _cursorIndexOfLastValidated = CursorUtil.getColumnIndexOrThrow(_cursor, "lastValidated");
          final int _cursorIndexOfLastUsed = CursorUtil.getColumnIndexOrThrow(_cursor, "lastUsed");
          final ApiKeyEntity _result;
          if (_cursor.moveToFirst()) {
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpKeyValue;
            _tmpKeyValue = _cursor.getString(_cursorIndexOfKeyValue);
            final boolean _tmpIsSet;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsSet);
            _tmpIsSet = _tmp != 0;
            final Long _tmpLastValidated;
            if (_cursor.isNull(_cursorIndexOfLastValidated)) {
              _tmpLastValidated = null;
            } else {
              _tmpLastValidated = _cursor.getLong(_cursorIndexOfLastValidated);
            }
            final Long _tmpLastUsed;
            if (_cursor.isNull(_cursorIndexOfLastUsed)) {
              _tmpLastUsed = null;
            } else {
              _tmpLastUsed = _cursor.getLong(_cursorIndexOfLastUsed);
            }
            _result = new ApiKeyEntity(_tmpProviderId,_tmpKeyValue,_tmpIsSet,_tmpLastValidated,_tmpLastUsed);
          } else {
            _result = null;
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
  public Flow<List<ApiKeyEntity>> getSetApiKeys() {
    final String _sql = "SELECT * FROM api_keys WHERE isSet = 1";
    final RoomSQLiteQuery _statement = RoomSQLiteQuery.acquire(_sql, 0);
    return CoroutinesRoom.createFlow(__db, false, new String[] {"api_keys"}, new Callable<List<ApiKeyEntity>>() {
      @Override
      @NonNull
      public List<ApiKeyEntity> call() throws Exception {
        final Cursor _cursor = DBUtil.query(__db, _statement, false, null);
        try {
          final int _cursorIndexOfProviderId = CursorUtil.getColumnIndexOrThrow(_cursor, "providerId");
          final int _cursorIndexOfKeyValue = CursorUtil.getColumnIndexOrThrow(_cursor, "keyValue");
          final int _cursorIndexOfIsSet = CursorUtil.getColumnIndexOrThrow(_cursor, "isSet");
          final int _cursorIndexOfLastValidated = CursorUtil.getColumnIndexOrThrow(_cursor, "lastValidated");
          final int _cursorIndexOfLastUsed = CursorUtil.getColumnIndexOrThrow(_cursor, "lastUsed");
          final List<ApiKeyEntity> _result = new ArrayList<ApiKeyEntity>(_cursor.getCount());
          while (_cursor.moveToNext()) {
            final ApiKeyEntity _item;
            final String _tmpProviderId;
            _tmpProviderId = _cursor.getString(_cursorIndexOfProviderId);
            final String _tmpKeyValue;
            _tmpKeyValue = _cursor.getString(_cursorIndexOfKeyValue);
            final boolean _tmpIsSet;
            final int _tmp;
            _tmp = _cursor.getInt(_cursorIndexOfIsSet);
            _tmpIsSet = _tmp != 0;
            final Long _tmpLastValidated;
            if (_cursor.isNull(_cursorIndexOfLastValidated)) {
              _tmpLastValidated = null;
            } else {
              _tmpLastValidated = _cursor.getLong(_cursorIndexOfLastValidated);
            }
            final Long _tmpLastUsed;
            if (_cursor.isNull(_cursorIndexOfLastUsed)) {
              _tmpLastUsed = null;
            } else {
              _tmpLastUsed = _cursor.getLong(_cursorIndexOfLastUsed);
            }
            _item = new ApiKeyEntity(_tmpProviderId,_tmpKeyValue,_tmpIsSet,_tmpLastValidated,_tmpLastUsed);
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

  @NonNull
  public static List<Class<?>> getRequiredConverters() {
    return Collections.emptyList();
  }
}
