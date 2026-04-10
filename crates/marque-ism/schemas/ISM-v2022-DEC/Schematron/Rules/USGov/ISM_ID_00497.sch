<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?><?schematron-phases phaseids="BANNER STRUCTURECHECK"?><!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00497">
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="ruleText">
        [ISM-ID-00497][Error] If a document contains either @ism:cuiBasic or @ism:cuiSpecified, 
        then the document must contain @ism:cuiControlledBy.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism"
          ism:classification="U"
          ism:ownerProducer="USA"
          class="codeDesc">
        If a document contains one or both of @ism:cuiBasic or @ism:cuiSpecified on the resource element, 
        this rule ensures that the document contains @ism:cuiControlledBy.
    </sch:p>
    <sch:rule id="ISM-ID-00497-R1"
             context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and (@ism:cuiBasic or @ism:cuiSpecified)]">
        <sch:assert test="@ism:cuiControlledBy" flag="error" role="error">
            [ISM-ID-00497][Error] If a document contains either @ism:cuiBasic or @ism:cuiSpecified, 
            then the document must contain @ism:cuiControlledBy.
        </sch:assert>
    </sch:rule>
</sch:pattern>
