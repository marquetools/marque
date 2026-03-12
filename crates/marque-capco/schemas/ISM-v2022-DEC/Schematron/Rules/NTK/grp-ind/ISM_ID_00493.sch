<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00493">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00493][Error] If a document contains the CUI dissemination marking [DL_ONLY], 
        it must contain an ntk:ProfileDes element with type ‘grp-ind’.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If a document contains the CUI dissemination marking [DL_ONLY], then there must be an ntk:ProfileDes element
        with value = ‘urn:us:gov:ic:ntk:profile:grp-ind’.
    </sch:p>
    <sch:rule id="ISM-ID-00493-R1" context="*[($ISM_USCUI_RESOURCE or $ISM_USCUIONLY_RESOURCE) and (generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)) and contains(./@ism:disseminationControls,'DL_ONLY')]">
        <sch:assert test="//ntk:ProfileDes[. = 'urn:us:gov:ic:ntk:profile:grp-ind']" flag="error" role="error">
            [ISM-ID-00493][Error] If a document contains the CUI dissemination marking [DL_ONLY], 
            it must contain an ntk:ProfileDes element with type ‘grp-ind’.</sch:assert>
    </sch:rule>
</sch:pattern>
