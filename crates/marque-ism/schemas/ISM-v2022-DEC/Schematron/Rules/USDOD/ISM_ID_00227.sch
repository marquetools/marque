<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       --><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00227">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00227][Error] Attribute @ism:noticeType may only appear on the 
        resource node when it contains the values [DoD-Dist-A], [DoD-Dist-B], 
        [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], [DoD-Dist-F], or [ITAR-EAR].
        
        Human Readable: Documents may only specify a document-level notice if
        it pertains to DoD Distribution.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For every resource element with the @ism:noticeType attribute specified,
        this rule ensures that attribute's value is one of [DoD-Dist-A], [DoD-Dist-B], 
        [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], [DoD-Dist-F], or [ITAR-EAR] by using a regular expression.
    </sch:p> 
    <sch:rule id="ISM-ID-00227-R1" context="*[generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT)  and @ism:noticeType]">       
        <sch:assert test="every $noticeToken in tokenize(normalize-space(string(@ism:noticeType)), ' ') satisfies                 
            matches($noticeToken, '^(DoD-Dist-[ABCDEF])|ITAR-EAR')" flag="error" role="error">
            [ISM-ID-00227][Error] Attribute @ism:noticeType may only appear on the 
            resource node when it contains the values [DoD-Dist-A], [DoD-Dist-B], 
            [DoD-Dist-C], [DoD-Dist-D], [DoD-Dist-E], [DoD-Dist-F], or [ITAR-EAR].
            
            Human Readable: Documents may only specify a document-level notice if
            it pertains to DoD Distribution.
        </sch:assert>
    </sch:rule>
</sch:pattern>