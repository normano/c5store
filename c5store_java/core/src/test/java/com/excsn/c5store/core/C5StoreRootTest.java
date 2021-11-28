package com.excsn.c5store.core;

import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;
import org.mockito.Mockito;

import java.util.Iterator;

@TestInstance(TestInstance.Lifecycle.PER_CLASS)
public class C5StoreRootTest {

  private C5StoreSubscriptions _c5StoreSubscriptions;
  private C5DataStore _c5DataStore;
  private C5StoreRoot _c5StoreRoot;

  @BeforeAll
  public void setup() throws Exception {

    _c5StoreSubscriptions = Mockito.mock(C5StoreSubscriptions.class);
    _c5DataStore = Mockito.mock(C5DataStore.class);

    _c5StoreRoot = new C5StoreRoot(
      _c5DataStore::getData,
      _c5DataStore::exists,
      _c5DataStore::keysWithPrefix,
      _c5StoreSubscriptions
    );
  }

  @Test
  public void getDataReturnsDatastoreData() {

    var expectedData = "foobar";
    Mockito.when(_c5DataStore.getData("test_key")).thenReturn(expectedData);
    var actualData = _c5StoreRoot.get("test_key");

    Assertions.assertEquals(actualData, expectedData);
  }

  @Test
  public void existsReturnsValid() {

    var expectedData = true;
    Mockito.when(_c5DataStore.exists("test_key")).thenReturn(expectedData);
    var actualData = _c5StoreRoot.exists("test_key");

    Assertions.assertTrue(actualData == expectedData);

    expectedData = false;
    Mockito.when(_c5DataStore.exists("test_key")).thenReturn(expectedData);
    actualData = _c5StoreRoot.exists("test_key");

    Assertions.assertTrue(actualData == expectedData);
  }

  @Test
  public void callsSubscribeSuccessfully() {

    var keyPath = "test_key";
    var changeSubscription = new ChangeListener() {
      @Override
      public void onChange(String notifyKeyPath, String keyPath, Object value) {

      }
    };

    Mockito.doNothing().when(_c5StoreSubscriptions).add(keyPath, changeSubscription);
    _c5StoreRoot.subscribe(keyPath, changeSubscription);

    Mockito.verify(_c5StoreSubscriptions, Mockito.times(1))
      .add(Mockito.eq(keyPath), Mockito.eq(changeSubscription));
  }

  @Test
  public void currentKeyPathReturnsNoneforRoot() {

    var expectedData = (String) null;
    var actualData = _c5StoreRoot.currentKeyPath();

    Assertions.assertEquals(expectedData, actualData);
  }

  @Test
  public void keyPathsWithPrefixReturnsIterator() {

    var keyPath = "test_key";
    var prefixIterator = new Iterator<String>() {
      @Override
      public boolean hasNext() {
        return false;
      }

      @Override
      public String next() {
        return null;
      }
    };

    Mockito.when(_c5DataStore.keysWithPrefix(keyPath)).thenReturn(prefixIterator);
    var expectedData = prefixIterator;
    var actualData = _c5StoreRoot.keyPathsWithPrefix(keyPath);

    Assertions.assertEquals(expectedData, actualData);
  }

  @Test
  public void branchReturnsABranchWithDesiredPrefix() {

    var keyPath = "test_key";
    var expectedData = keyPath;
    var actualData = _c5StoreRoot.branch(keyPath).currentKeyPath();

    Assertions.assertEquals(expectedData, actualData);
  }

  @Test
  public void branchedGetReturnsDatastoreData() {

    var keyPath = "test_key";
    var expectedData = "foobar";
    Mockito.when(_c5DataStore.getData("test_key.beep")).thenReturn(expectedData);

    var branch = _c5StoreRoot.branch(keyPath);
    var actualData = branch.get("beep");

    Assertions.assertEquals(expectedData, actualData);
  }
}
