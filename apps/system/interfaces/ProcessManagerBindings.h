#ifndef __PROCESS_MANGER_BINDINGS_H__
#define __PROCESS_MANGER_BINDINGS_H__

/* Warning, this file is autogenerated by cbindgen. Don't modify this manually. */

#define MAX_BUNDLES 10

#define MAX_BUNDLE_ID_SIZE 32

typedef struct Bundle {
  uint32_t something;
} Bundle;

typedef struct BundleId {
  uint8_t id[MAX_BUNDLE_ID_SIZE];
} BundleId;

typedef struct BundleIdArray {
  struct BundleId ids[MAX_BUNDLES];
} BundleIdArray;

#endif /* __PROCESS_MANGER_BINDINGS_H__ */
