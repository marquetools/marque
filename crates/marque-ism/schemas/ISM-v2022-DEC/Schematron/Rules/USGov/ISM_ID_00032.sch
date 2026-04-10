<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00032">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00032][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls is not specified, or is specified and does not 
        contain the name token [REL] or [EYES], then attribute @ism:releasableTo 
        must not be specified.
        
        Human Readable: USA documents must only specify to which countries it is 
        authorized for release if dissemination information contains 
        REL TO or EYES ONLY data. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        does not specify attribute @ism:disseminationControls or specifies attribute
        @ism:disseminationControls with a value containing the token 
        [REL] or [EYES] this rule ensures that attribute @ism:releasableTo is not 
        specified.
    </sch:p>
    <sch:rule id="ISM-ID-00032-R1" context="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('REL', 'EYES')))]">
        <sch:assert test="not(@ism:releasableTo)" flag="error" role="error">
            [ISM-ID-00032][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls is not specified, or is specified and does not 
            contain the name token [REL] or [EYES], then attribute @ism:releasableTo 
            must not be specified.
            
            Human Readable: USA documents must only specify to which countries it is 
            authorized for release if dissemination information contains 
            REL TO or EYES ONLY data. 
        </sch:assert>
    </sch:rule>
</sch:pattern>