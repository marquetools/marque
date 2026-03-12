<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00339">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00339][Error] 
        1. @ism:ownerProducer of resource element contains USA
        2. @ism:compliesWith does not contain USGov  
        
        Human Readable: All documents that contain USA in @ism:ownerProducer of
        the first resource node (in document order) must claim USGov in @ism:compliesWith
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If a document contains USA in @ism:ownerProducer (for the resource element), then
        @ism:compliesWith must contain USGov.
    </sch:p>
    <sch:rule id="ISM-ID-00339-R1" context="*[ generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:ownerProducer, ('USA'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:compliesWith, ('USGov'))" flag="error" role="error"> 
            [ISM-ID-00339][Error] 
            1. ism:ownerProducer of resource element contains USA
            2. ism:compliesWith does not contain USGov
            
            Human Readable: All documents that contain USA in @ism:ownerProducer of
            the first resource node (in document order) must claim USGov in @ism:compliesWith
        </sch:assert>
    </sch:rule>
</sch:pattern>