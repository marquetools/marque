<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00225">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00225][Error] If subject to IC rules, then attribute @ism:nonICmarkings must NOT be specified 
        with a value containing any name token starting with [ACCM] or [NNPI]. 
        
        Human Readable: ACCM and NNPI tokens are not valid for documents that are subject to IC rules.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If ISM_USIC_RESOURCE, for each element which has attribute @ism:nonICmarkings specified, this rule ensures that
        attribute @ism:nonICmarkings is not specified with a value containing a token which starts with [ACCM] or [NNPI].
    </sch:p>
    <sch:rule id="ISM-ID-00225-R1" context="*[$ISM_USIC_RESOURCE and @ism:nonICmarkings and util:contributesToRollup(.)]">
        <sch:assert test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, ('ACCM', 'NNPI')))" flag="error" role="error">
            [ISM-ID-00225][Error]  If subject to IC rules, then attribute @ism:nonICmarkings must NOT be specified 
            with a value containing any name token starting with [ACCM] or [NNPI]. 
            
            Human Readable: ACCM and NNPI tokens are not valid for documents that are subject to IC rules.
        </sch:assert>
    </sch:rule>
</sch:pattern>