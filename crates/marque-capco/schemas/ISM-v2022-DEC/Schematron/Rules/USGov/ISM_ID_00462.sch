<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00462">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00462][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification is [U], then attribute @ism:nonICmarkings
        must not contain a name token that starts with ACCM.
        
        Human Readable: A USA document containing ACCM data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which specifies attribute @ism:classification='U', 
        then this rule ensures that @ism:nonICmarkings does not contain a token that starts with ACCM.
    </sch:p>
    <sch:rule id="ISM-ID-00462-R1" context="*[$ISM_USGOV_RESOURCE and util:contributesToRollup(.) and @ism:classification='U']">
        <sch:assert test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, ('ACCM')))" flag="error" role="error">
            [ISM-ID-00462][Error] If ISM_USGOV_RESOURCE and attribute @ism:classification is [U], then attribute @ism:nonICmarkings
            must not contain a name token that starts with ACCM.
            
            Human Readable: A USA document containing ACCM data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.
        </sch:assert>
    </sch:rule>
</sch:pattern>
