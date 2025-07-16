# 版本配置
VERSION = 1.0.4
PACKAGE_NAME = wuhu_go_$(VERSION)
DIST_DIR = dist/$(PACKAGE_NAME)

BUILD_FLAGS = -ldflags="-w -s -X 'main.Version=v$(VERSION)'"
RUN_FLAGS = -ldflags="-X 'main.Version=v$(VERSION)'"

run-wuhu:
	cd src/wuhu && go run $(RUN_FLAGS) wuhu.go

run-ding:
	cd src/ding && go run ding.go

run-yee:
	cd src/yee && go run yee.go

build-wuhu:
	mkdir -p $(DIST_DIR)
	cd src/wuhu && GOOS=windows GOARCH=amd64 go build $(BUILD_FLAGS) -o ../../$(DIST_DIR)/wuhu.exe wuhu.go

build-ding:
	mkdir -p $(DIST_DIR)
	cd src/ding && GOOS=windows GOARCH=amd64 go build $(BUILD_FLAGS) -o ../../$(DIST_DIR)/ding.exe ding.go

build-yee:
	mkdir -p $(DIST_DIR)
	cd src/yee && GOOS=windows GOARCH=amd64 go build $(BUILD_FLAGS) -o ../../$(DIST_DIR)/yee.exe yee.go

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

package: build
	cd dist && tar -czf $(PACKAGE_NAME).tar.gz $(PACKAGE_NAME)/
	@echo "Package completed: dist/$(PACKAGE_NAME).tar.gz"

.PHONY: run-wuhu run-ding run-yee build-wuhu build-ding build-yee copy-assets build clean package 