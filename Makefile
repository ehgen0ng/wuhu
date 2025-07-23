# 版本配置
VERSION = 1.1.7
PACKAGE_NAME = wuhu_go_$(VERSION)
DIST_DIR = dist/$(PACKAGE_NAME)

BUILD_FLAGS = -ldflags="-w -s -X 'main.Version=v$(VERSION)'"
RUN_FLAGS = -ldflags="-X 'main.Version=v$(VERSION)'"

# yee 项目特殊配置
YEE_SDK_PATH = src/yee/steamworks_sdk_162/sdk
YEE_OBJ_FILE = src/yee/steam_init.o
YEE_CPP_FILE = src/yee/steam_init.cpp

run-wuhu:
	cd src/wuhu && go run $(RUN_FLAGS) wuhu.go

run-ding:
	cd src/ding && go run ding.go

run-yee: $(YEE_OBJ_FILE)
	cd src/yee && go run yee.go

build-wuhu:
	mkdir -p $(DIST_DIR)
	cd src/wuhu && GOOS=windows GOARCH=amd64 go build $(BUILD_FLAGS) -o ../../$(DIST_DIR)/wuhu.exe wuhu.go

build-ding:
	mkdir -p $(DIST_DIR)
	cd src/ding && GOOS=windows GOARCH=amd64 go build $(BUILD_FLAGS) -o ../../$(DIST_DIR)/ding.exe ding.go

build-yee: $(YEE_OBJ_FILE)
	mkdir -p $(DIST_DIR)
	@echo Cross-compiling yee.exe for Windows...
	cd src/yee && CGO_ENABLED=1 GOOS=windows GOARCH=amd64 CC=x86_64-w64-mingw32-gcc CXX=x86_64-w64-mingw32-g++ go build $(BUILD_FLAGS) -o ../../$(DIST_DIR)/yee.exe yee.go
	@echo Copying steam_api64.dll to dist...
	@cp "$(YEE_SDK_PATH)/redistributable_bin/win64/steam_api64.dll" "$(DIST_DIR)/steam_api64.dll"

# 编译 yee 项目的 C++ 对象文件
$(YEE_OBJ_FILE): $(YEE_CPP_FILE) src/yee/steam_init.h
	@echo Cross-compiling C++ source for Windows...
	x86_64-w64-mingw32-g++ -c $(YEE_CPP_FILE) -I$(YEE_SDK_PATH)/public -o $(YEE_OBJ_FILE)



copy-assets:
	cp -r utils $(DIST_DIR)/
	cp -r List $(DIST_DIR)/
	cp README_冲.md $(DIST_DIR)/
	cp LICENSE $(DIST_DIR)/
	find $(DIST_DIR) -name ".DS_Store" -delete

build: build-wuhu build-ding build-yee copy-assets
	@echo "Build completed: $(DIST_DIR)/"
	@ls -la $(DIST_DIR)/

clean:
	rm -rf dist/
	rm -f $(YEE_OBJ_FILE)

.PHONY: run-wuhu run-ding run-yee build-wuhu build-ding build-yee copy-assets build clean package install-yee-deps 