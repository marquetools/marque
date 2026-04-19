<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00486">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00486][Error] If ISM_USCUIONLY_RESOURCE or ISM_USCUI_RESOURCE then attribute @ism:nonICmarkings must not be specified.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is ISM_USCUIONLY_RESOURCE or ISM_USCUI_RESOURCE, this rule ensures that @ism:nonICmarkings 
        does not appear in the document.
    </sch:p>
    <sch:rule id="ISM-ID-00486-R1" context="*[($ISM_USCUIONLY_RESOURCE or $ISM_USCUI_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT))]">
        <sch:assert test="not(*/@ism:nonICmarkings)" flag="error" role="error">
            [ISM-ID-00486][Error] If ISM_USCUIONLY_RESOURCE or ISM_USCUI_RESOURCE then attribute @ism:nonICmarkings must not be specified.
        </sch:assert>
    </sch:rule>
</sch:pattern>